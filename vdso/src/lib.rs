//! 这里用于内核初始化 vDSO

#![no_std]
#![feature(noop_waker)]

extern crate alloc;

use alloc::{boxed::Box, collections::btree_set::Union, sync::Arc, vec::Vec};
use async_mem::MemorySet;
use axalloc::PhysPage;
use axhal::{
    mem::{phys_to_virt, virt_to_phys},
    paging::MappingFlags,
};
use core::{
    cell::UnsafeCell,
    future::Future,
    hint::black_box,
    ptr::copy_nonoverlapping,
    task::{Context, Poll, Waker},
};
use elf_parser::get_relocate_pairs;
use include_bytes_aligned::include_bytes_aligned;
use lazy_init::LazyInit;
use log::{info, warn};
use memory_addr::{VirtAddr, PAGE_SIZE_4K};
use sync::Mutex;

use libvdsoexample as vdso_lib;
use vdso_lib::{PhysPagePtr, VvarData};

static SO_CONTENT: &[u8] = include_bytes_aligned!(8, "../../vdso_output/libvdsoexample.so");
const VDSO_SIZE: usize = ((SO_CONTENT.len() - 1) / PAGE_SIZE_4K + 1) * PAGE_SIZE_4K + PAGE_SIZE_4K; // 额外加了一页，用于bss段等未出现在文件中的段

pub fn init(k_memset: &mut MemorySet) {
    // VDSO_INFO.init_by(VdsoInfo::new());
    vdso_lib::load_and_init(k_memset as *mut MemorySet as usize);
    unsafe {
        test_vdso();
    }
}

/// 返回值：vDSO在用户空间的基地址
pub fn map(memset: &mut MemorySet) -> VirtAddr {
    // VDSO_INFO.init_by(VdsoInfo::new());
    let vdso_base = vdso_lib::map_so(memset as *mut MemorySet as usize);
    VirtAddr::from(vdso_base as usize)
}

const VVAR_PAGES: usize = (size_of::<VvarData>() - 1) / PAGE_SIZE_4K + 1;
const VVAR_SIZE: usize = VVAR_PAGES * PAGE_SIZE_4K;

struct SyncUnsafeCell<T>(UnsafeCell<T>);
unsafe impl<T> Sync for SyncUnsafeCell<T> {}

#[link_section = ".vvar"]
#[no_mangle]
static VVAR: SyncUnsafeCell<[u8; VVAR_SIZE]> = SyncUnsafeCell(UnsafeCell::new([0; VVAR_SIZE]));
#[link_section = ".vdso"]
#[no_mangle]
static VDSO: SyncUnsafeCell<[u8; VDSO_SIZE]> = SyncUnsafeCell(UnsafeCell::new([0; VDSO_SIZE]));

pub static VDSO_INFO: LazyInit<VdsoInfo> = LazyInit::new();

pub struct VdsoInfo {
    pub name: &'static str,
    pub elf_data: &'static [u8],
    pub cm: Vec<PhysPage>,
}

impl VdsoInfo {
    pub fn new() -> Self {
        info!("Initialize vDSO...");
        // 加载vvar区域
        unsafe {
            (VVAR.0.get() as *mut () as *mut VvarData).write(VvarData::default());
        }
        // 加载vdso区域
        let vdso_start: usize = &VDSO as *const _ as usize;
        let vdso_end: usize = vdso_start + VDSO_SIZE;
        load_vdso(vdso_start, vdso_start);

        let len = vdso_end as usize - vdso_start as usize;
        let start = vdso_start as usize;
        let pages = len / PAGE_SIZE_4K;
        // assert_eq!(&elf_data[0..4], b"\x7fELF");
        let cm = (0..pages)
            .map(|i| PhysPage {
                start_vaddr: (start as usize + i * PAGE_SIZE_4K).into(),
            })
            .collect::<Vec<PhysPage>>();

        unsafe {
            vdso_lib::init_vdso_vtable(vdso_start as u64);
        }

        let elf_data = unsafe { core::slice::from_raw_parts(start as *const u8, len) };
        Self {
            name: "vdso",
            elf_data,
            cm,
        }
    }

    pub async fn vdso2memoryset(&self, memory_set: &mut MemorySet) -> VirtAddr {
        log::warn!("Mapping vDSO to memory set...");
        let vvar_base = memory_set.max_va();
        let vdso_base = vvar_base + VVAR_SIZE;

        // 映射 vDSO数据区域
        let vvar_paddr = (&VVAR as *const _ as usize) - axconfig::KERNEL_BASE_VADDR
            + axconfig::KERNEL_BASE_PADDR;
        let _ = memory_set
            .map_attach_shared_page_without_alloc(
                vvar_base,
                vvar_paddr.into(),
                VVAR_PAGES,
                MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
            )
            .await;
        log::warn!("vVAR mapped at {:#x}", vvar_base.as_usize());

        // 映射 vDSO 代码区域
        // 此处使用SO_CONTENT而非VDSO，因为内核可能已经修改的VDSO。
        // 暂时使用“直接拷贝到用户区域”代替“共享页面+写时复制”，确保各个地址空间的vDSO代码区域互不影响。
        // 未来可以改为共享页面+写时复制以提高性能。
        let _ = memory_set
            .new_region(
                vdso_base,
                VDSO_SIZE,
                false,
                MappingFlags::READ
                    | MappingFlags::WRITE
                    | MappingFlags::EXECUTE
                    | MappingFlags::USER,
                Some(&[0u8; VDSO_SIZE]),
                None,
            )
            .await;
        let kernel_vaddr = memory_set.query(vdso_base).unwrap().0.as_usize()
            - axconfig::KERNEL_BASE_PADDR
            + axconfig::KERNEL_BASE_VADDR;
        load_vdso(kernel_vaddr, vdso_base.as_usize());

        log::warn!("vDSO mapped at {:#x} in userspace", vdso_base.as_usize());

        vdso_base
    }
}

pub fn load_vdso(curr_vspace_addr: usize, target_vspace_addr: usize) {
    let elf = xmas_elf::ElfFile::new(&SO_CONTENT).expect("Error parsing vDSO.");
    let segments = elf_parser::get_elf_segments(&elf, Some(curr_vspace_addr));
    for segment in segments {
        if let Some(src) = segment.data {
            let dst = segment.vaddr.as_mut_ptr();
            unsafe {
                core::ptr::copy_nonoverlapping(src.as_ptr(), dst, segment.size);
            }
        }

        log::info!(
            "Load vDSO segment: vaddr=0x{:x}, size=0x{:x}, flags={:?}",
            segment.vaddr,
            segment.size,
            segment.flags
        );
    }
    let relocate_pairs = elf_parser::get_relocate_pairs(&elf, Some(target_vspace_addr));
    for relocate_pair in relocate_pairs {
        let src: usize = relocate_pair.src.into();
        let dst: usize = relocate_pair.dst.into();
        let count = relocate_pair.count;
        log::info!(
            "Relocate: src: 0x{:x}, dst: 0x{:x}, count: {}",
            src,
            dst,
            count
        );
        unsafe { core::ptr::copy_nonoverlapping(src.to_ne_bytes().as_ptr(), dst as *mut u8, count) }
    }

    log::warn!(
        "vDSO loaded at {:#x} in current vspace, {:#x} in target vspace.",
        curr_vspace_addr,
        target_vspace_addr
    );
}

/// SAFETY: 调用该函数前需要先调用vdso_lib::init_vdso_vtable。
pub unsafe fn test_vdso() {
    warn!("Testing vDSO in kernel...");
    assert_eq!(vdso_lib::get_shared().i, 0);
    vdso_lib::set_shared(1);
    assert_eq!(vdso_lib::get_shared().i, 1);
    assert_eq!(vdso_lib::get_private().i, 0);
    vdso_lib::set_private(1);
    assert_eq!(vdso_lib::get_private().i, 1);
    warn!("Test passed!");
}

// 这以下是新版接口的实现

/// vspace：对应由&mut MemorySet转化而来的*mut MemorySet。
///
/// PhysPagePtr：对应由Arc<Vec<Mutex<PhysPage>>>转化而来的*const Vec<Mutex<PhysPage>>。需要注意在这些函数中不要消耗引用计数。
struct MemIfImpl;

#[crate_interface::impl_interface]
impl vdso_lib::MemIf for MemIfImpl {
    #[doc = " 在地址空间中分配用于vDSO和vVAR的虚存区域（不需同时分配物理页面），返回指向首地址的指针。"]
    #[doc = " "]
    #[doc = " 保证size为build_vdso传入的config.page_size的整数倍。"]
    #[doc = " 要求返回的地址也为config.page_size的整数倍。"]
    fn valloc(vspace: usize, size: usize) -> *mut u8 {
        if vspace == 0 {
            // 内核空间直接使用静态地址，不需要分配新的虚存区域。
            // 这里的vspace参数被忽略了。
            &VVAR as *const _ as *const () as *mut u8
        } else {
            let memory_set = unsafe { &*(vspace as *const MemorySet) };
            let vaddr = memory_set.find_free_area(VirtAddr::from(0), size);
            vaddr.unwrap().as_mut_ptr()
        }
    }

    #[doc = " 分配多块用于vDSO和vVAR的连续物理页，返回`PhysPagePtr`。"]
    #[doc = " "]
    #[doc = " 保证size为build_vdso传入的config.page_size的整数倍。"]
    #[doc = ""]
    #[doc = " 若需要实现vDSO和vVAR在多地址空间的共享，则需要在分配时使这块空间可被共享（即，可被多次`map`）。"]
    fn ppage_alloc(size: usize) -> PhysPagePtr {
        assert_eq!(size % PAGE_SIZE_4K, 0);
        let pages = Arc::new(
            PhysPage::alloc_contiguous(size / PAGE_SIZE_4K, PAGE_SIZE_4K, None)
                .unwrap()
                .into_iter()
                .map(|page| Mutex::new(page.unwrap()))
                .collect::<Vec<_>>(),
        );
        Arc::into_raw(pages) as usize
    }

    #[doc = " 从`alloc`返回的虚存区域中，映射其中一块到某个物理页面并设置权限。"]
    #[doc = " "]
    #[doc = " 被映射的物理页面可能和其它地址空间共享，也可能由这个地址空间独占。"]
    #[doc = " "]
    #[doc = " 保证vaddr对齐到build_vdso传入的config.page_size；len为config.page_size的整数倍。"]
    #[doc = ""]
    #[doc = " `flags`可能包含：READ、WRITE、EXECUTE、USER。"]
    fn map(
        vspace: usize,
        vaddr: *mut u8,
        ppage: PhysPagePtr,
        size: usize,
        flags: vdso_lib::MappingFlags,
    ) {
        let memory_set = unsafe { &mut *(vspace as *mut MemorySet) };
        let ppage_vec = unsafe { Arc::from_raw(ppage as *const Vec<Mutex<PhysPage>>) };
        let paddr = virt_to_phys(ppage_vec[0].lock().start_vaddr);
        let flags_1 = MappingFlags::from_bits(flags.bits()).unwrap(); // 两个仓库的MappingFlags版本不同，但它们的bits是一样的，所以可以通过bits转换一下。
        block_on(memory_set.map_attach_shared_page_without_alloc(
            VirtAddr::from(vaddr as usize),
            paddr,
            size / PAGE_SIZE_4K,
            flags_1,
        ))
        .unwrap();
        Arc::into_raw(ppage_vec);
    }

    #[doc = " 重新设置已映射好的，虚拟首地址为`vspace`区域的权限。"]
    #[doc = " "]
    #[doc = " 保证vaddr对齐到build_vdso传入的config.page_size。"]
    fn change_protect(vspace: usize, vaddr: *mut u8, size: usize, flags: vdso_lib::MappingFlags) {
        let memory_set = unsafe { &mut *(vspace as *mut MemorySet) };
        let flags_1 = MappingFlags::from_bits(flags.bits()).unwrap(); // 两个仓库的MappingFlags版本不同，但它们的bits是一样的，所以可以通过bits转换一下。
        block_on(memory_set.mprotect(VirtAddr::from(vaddr as usize), size, flags_1));
    }

    #[doc = " 获取`vspace`空间中`vaddr`地址对应的内核虚拟地址。"]
    #[doc = " （也就是当前代码可以直接访问的地址）"]
    fn get_kernel_vaddr(vspace: usize, vaddr: *mut u8) -> *mut u8 {
        let memory_set = unsafe { &*(vspace as *const MemorySet) };
        let (paddr, _, _) = memory_set.query(VirtAddr::from(vaddr as usize)).unwrap();
        let vaddr = phys_to_virt(paddr);
        vaddr.as_mut_ptr()
    }

    #[doc = " 复制物理页指针，复制前后指向同一块物理页。复制后，参数和返回值对应的两个指针均需可用。"]
    #[doc = " "]
    #[doc = " 如果物理页使用RAII管理，则需调用其`clone`方法。"]
    #[doc = " "]
    #[doc = " 如果物理页不使用RAII管理，则可以直接返回参数。"]
    fn ppage_clone(ppage: PhysPagePtr) -> PhysPagePtr {
        let ppage_vec = unsafe { Arc::from_raw(ppage as *const Vec<Mutex<PhysPage>>) };
        let cloned_ppage_vec = ppage_vec.clone();
        Arc::into_raw(ppage_vec);
        Arc::into_raw(cloned_ppage_vec) as usize
    }
}

fn block_on<F: Future>(fut: F) -> F::Output {
    let mut pinned_fut = Box::pin(fut);
    loop {
        if let Poll::Ready(output) = pinned_fut
            .as_mut()
            .poll(&mut Context::from_waker(&Waker::noop()))
        {
            return output;
        }
    }
}
