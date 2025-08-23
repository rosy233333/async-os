//! 这里用于内核初始化 vDSO

#![no_std]

extern crate alloc;

use alloc::{collections::btree_set::Union, vec::Vec};
use async_mem::MemorySet;
use axalloc::PhysPage;
use axhal::paging::MappingFlags;
use core::{cell::UnsafeCell, hint::black_box, ptr::copy_nonoverlapping};
use elf_parser::get_relocate_pairs;
use lazy_init::LazyInit;
use log::{info, warn};
use memory_addr::{VirtAddr, PAGE_SIZE_4K};

static SO_CONTENT: &[u8] = include_bytes!("../libvdsoexample.so");
const VDSO_SIZE: usize = ((SO_CONTENT.len() - 1) / PAGE_SIZE_4K + 1) * PAGE_SIZE_4K;

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
        black_box(&VVAR); // 避免VVAR区域被编译器优化掉
        unsafe {
            (&mut *VDSO.0.get())[0..SO_CONTENT.len()].copy_from_slice(SO_CONTENT);
        }

        let vdso_start: usize = &VDSO as *const _ as usize;
        let vdso_end: usize = vdso_start + VDSO_SIZE;

        let len = vdso_end as usize - vdso_start as usize;
        let start = vdso_start as usize;
        let pages = len / PAGE_SIZE_4K;
        let elf_data = unsafe { core::slice::from_raw_parts(start as *const u8, len) };
        assert_eq!(&elf_data[0..4], b"\x7fELF");
        let cm = (0..pages)
            .map(|i| PhysPage {
                start_vaddr: (start as usize + i * PAGE_SIZE_4K).into(),
            })
            .collect::<Vec<PhysPage>>();

        let elf = xmas_elf::ElfFile::new(elf_data).expect("Error parsing vDSO.");
        unsafe {
            api::init_vdso_vtable(vdso_start as u64, &elf);
        }
        api::init();

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
                Some(&SO_CONTENT),
                None,
            )
            .await;
        log::warn!("vDSO mapped at {:#x}", vdso_base.as_usize());

        vdso_base
    }
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
