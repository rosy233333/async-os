//! 这里用于内核初始化 vDSO

#![no_std]

extern crate alloc;

use alloc::{collections::btree_set::Union, vec::Vec};
use async_mem::MemorySet;
use axalloc::PhysPage;
use axhal::paging::MappingFlags;
use core::{cell::UnsafeCell, hint::black_box, ptr::copy_nonoverlapping};
use elf_parser::get_relocate_pairs;
use include_bytes_aligned::include_bytes_aligned;
use lazy_init::LazyInit;
use log::{info, warn};
use memory_addr::{VirtAddr, PAGE_SIZE_4K};
use structs::shared::VvarData;

static SO_CONTENT: &[u8] = include_bytes_aligned!(8, "../libvdsoexample.so");
const VDSO_SIZE: usize = ((SO_CONTENT.len() - 1) / PAGE_SIZE_4K + 1) * PAGE_SIZE_4K + PAGE_SIZE_4K; // 额外加了一页，用于bss段等未出现在文件中的段

pub fn init() {
    VDSO_INFO.init_by(VdsoInfo::new());
    unsafe {
        test_vdso();
    }
}

const VVAR_PAGES: usize = (api::VVAR_DATA_SIZE - 1) / PAGE_SIZE_4K + 1;
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
            (VVAR.0.get() as *mut () as *mut VvarData).write(VvarData::new());
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
            api::init_vdso_vtable(vdso_start as u64);
        }
        // api::init();

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

/// SAFETY: 调用该函数前需要先调用api::init_vdso_vtable。
pub unsafe fn test_vdso() {
    warn!("Testing vDSO in kernel...");
    assert_eq!(api::get_shared().i, 42);
    api::set_shared(1);
    assert_eq!(api::get_shared().i, 1);
    assert_eq!(api::get_private().i, 0);
    api::set_private(1);
    assert_eq!(api::get_private().i, 1);
    warn!("Test passed!");
}
