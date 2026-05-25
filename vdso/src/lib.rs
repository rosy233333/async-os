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
    mem,
    ptr::copy_nonoverlapping,
    sync::atomic::{AtomicUsize, Ordering},
    task::{Context, Poll, Waker},
};
use elf_parser::get_relocate_pairs;
use include_bytes_aligned::include_bytes_aligned;
use lazy_init::LazyInit;
use log::{info, warn};
use memory_addr::{PhysAddr, VirtAddr, PAGE_SIZE_4K};
use sync::Mutex;

use libvdsoexample as vdso_lib;
use vdso_lib::{PhysPagePtr, VvarData};

static SO_CONTENT: &[u8] = include_bytes_aligned!(8, "../../vdso_output/libvdsoexample.so");
const VDSO_SIZE: usize = ((SO_CONTENT.len() - 1) / PAGE_SIZE_4K + 1) * PAGE_SIZE_4K + PAGE_SIZE_4K; // 额外加了一页，用于bss段等未出现在文件中的段

pub fn init() {
    // VDSO_INFO.init_by(VdsoInfo::new());
    // vdso_lib::load_and_init(k_memset as *mut MemorySet as usize);
    // 避免两个全局变量被编译器优化。
    unsafe {
        (*VVAR.0.get())[0] = 0;
        (*VDSO.0.get())[0] = 0;
    }
    vdso_lib::load_and_init(0);
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
#[used]
static VVAR: SyncUnsafeCell<[u8; VVAR_SIZE]> = SyncUnsafeCell(UnsafeCell::new([0; VVAR_SIZE]));
#[link_section = ".vdso"]
#[no_mangle]
#[used]
static VDSO: SyncUnsafeCell<[u8; VDSO_SIZE]> = SyncUnsafeCell(UnsafeCell::new([0; VDSO_SIZE]));

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
/// PhysPagePtr：物理页在内核空间的首地址
struct MemIfImpl;

static CURRENT_PTR: AtomicUsize = AtomicUsize::new(0);

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
            CURRENT_PTR.store(&VVAR as *const _ as usize, Ordering::Release);
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
        // let pages = Arc::new(
        //     PhysPage::alloc_contiguous(size / PAGE_SIZE_4K, PAGE_SIZE_4K, None)
        //         .unwrap()
        //         .into_iter()
        //         .map(|page| Mutex::new(page.unwrap()))
        //         .collect::<Vec<_>>(),
        // );
        // Arc::into_raw(pages) as usize
        let current_ptr_end = &VVAR as *const _ as usize + VVAR_SIZE + VDSO_SIZE;
        if CURRENT_PTR.load(Ordering::Acquire) < current_ptr_end {
            // 内核空间直接使用静态分配的物理页，不需要分配新的物理页。
            let vaddr = CURRENT_PTR.fetch_add(size, Ordering::AcqRel);
            assert!(CURRENT_PTR.load(Ordering::Acquire) <= current_ptr_end);
            virt_to_phys(VirtAddr::from(vaddr)).into()
        } else {
            // 为用户空间分配物理页。
            let pages =
                PhysPage::alloc_contiguous(size / PAGE_SIZE_4K, PAGE_SIZE_4K, None).unwrap();
            let paddr = virt_to_phys(pages[0].as_ref().unwrap().start_vaddr);
            mem::forget(pages);
            paddr.into()
        }
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
        if vspace != 0 {
            let memory_set = unsafe { &mut *(vspace as *mut MemorySet) };
            let paddr = PhysAddr::from(ppage);
            let flags_1 = MappingFlags::from_bits(flags.bits()).unwrap(); // 两个仓库的MappingFlags版本不同，但它们的bits是一样的，所以可以通过bits转换一下。
            let shared_page_start = virt_to_phys(VirtAddr::from(&VVAR as *const _ as usize));
            let shared_page_end = shared_page_start + VVAR_SIZE + VDSO_SIZE;
            if paddr >= shared_page_start && paddr < shared_page_end {
                // 需要共享
                block_on(memory_set.map_attach_shared_page_without_alloc(
                    VirtAddr::from(vaddr as usize),
                    paddr,
                    size / PAGE_SIZE_4K,
                    flags_1,
                ))
                .unwrap();
            } else {
                // 不需要共享
                block_on(memory_set.map_attach_page_without_alloc(
                    VirtAddr::from(vaddr as usize),
                    paddr,
                    size / PAGE_SIZE_4K,
                    flags_1,
                ))
                .unwrap();
            }
            // Arc::into_raw(ppage_vec);
        }
    }

    #[doc = " 重新设置已映射好的，虚拟首地址为`vspace`区域的权限。"]
    #[doc = " "]
    #[doc = " 保证vaddr对齐到build_vdso传入的config.page_size。"]
    fn change_protect(vspace: usize, vaddr: *mut u8, size: usize, flags: vdso_lib::MappingFlags) {
        if vspace != 0 {
            let memory_set = unsafe { &mut *(vspace as *mut MemorySet) };
            let flags_1 = MappingFlags::from_bits(flags.bits()).unwrap(); // 两个仓库的MappingFlags版本不同，但它们的bits是一样的，所以可以通过bits转换一下。
            block_on(memory_set.mprotect(VirtAddr::from(vaddr as usize), size, flags_1));
        }
    }

    #[doc = " 获取`vspace`空间中`vaddr`地址对应的内核虚拟地址。"]
    #[doc = " （也就是当前代码可以直接访问的地址）"]
    fn get_kernel_vaddr(vspace: usize, vaddr: *mut u8) -> *mut u8 {
        if vspace == 0 {
            vaddr
        } else {
            let memory_set = unsafe { &*(vspace as *const MemorySet) };
            let (paddr, _, _) = memory_set.query(VirtAddr::from(vaddr as usize)).unwrap();
            let vaddr = phys_to_virt(paddr);
            vaddr.as_mut_ptr()
        }
    }

    #[doc = " 复制物理页指针，复制前后指向同一块物理页。复制后，参数和返回值对应的两个指针均需可用。"]
    #[doc = " "]
    #[doc = " 如果物理页使用RAII管理，则需调用其`clone`方法。"]
    #[doc = " "]
    #[doc = " 如果物理页不使用RAII管理，则可以直接返回参数。"]
    fn ppage_clone(ppage: PhysPagePtr) -> PhysPagePtr {
        // let ppage_vec = unsafe { Arc::from_raw(ppage as *const Vec<Mutex<PhysPage>>) };
        // let cloned_ppage_vec = ppage_vec.clone();
        // Arc::into_raw(ppage_vec);
        // Arc::into_raw(cloned_ppage_vec) as usize
        ppage
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
