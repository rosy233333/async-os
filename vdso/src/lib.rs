//! 这里用于内核初始化 vDSO

#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use async_mem::MemorySet;
use axalloc::PhysPage;
use axhal::paging::MappingFlags;
use core::ptr::copy_nonoverlapping;
use elf_parser::get_relocate_pairs;
use lazy_init::LazyInit;
use memory_addr::{VirtAddr, PAGE_SIZE_4K};

core::arch::global_asm!(include_str!("vdso.S"));

pub fn init() {
    VDSO_INFO.init_by(VdsoInfo::new());
}

extern "C" {
    fn vdso_start();
    fn vdso_end();
}

static VVAR_PAGES: usize = 2;
static VVAR_SIZE: usize = VVAR_PAGES * PAGE_SIZE_4K;

pub static VDSO_INFO: LazyInit<VdsoInfo> = LazyInit::new();

pub struct VdsoInfo {
    pub name: &'static str,
    pub elf_data: &'static [u8],
    pub cm: Vec<PhysPage>,
}

impl VdsoInfo {
    pub fn new() -> Self {
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
        Self {
            name: "vdso",
            elf_data,
            cm,
        }
    }

    pub async fn vdso2memoryset(&self, memory_set: &mut MemorySet) -> VirtAddr {
        let vvar_base = memory_set.max_va();
        let vdso_base = vvar_base + VVAR_SIZE;
        // 映射 vdso 数据区域
        let _ = memory_set
            .new_region(
                vvar_base,
                VVAR_SIZE,
                false,
                MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
                None,
                None,
            )
            .await;
        let elf = xmas_elf::ElfFile::new(self.elf_data).expect("Error parsing vDSO.");
        let relocate_pairs = get_relocate_pairs(&elf, Some(vdso_base.as_usize()));
        for relocate_pair in relocate_pairs {
            let src: usize = relocate_pair.src.into();
            let dst: usize = relocate_pair.dst.into();
            let count = relocate_pair.count;
            log::error!("src: {:#x}, dst: {:#x}, count: {:#x}", src, dst, count);
            unsafe { copy_nonoverlapping(src.to_ne_bytes().as_ptr(), dst as *mut u8, count) }
        }
        // 映射 vDSO 代码区域
        for (idx, page) in self.cm.iter().enumerate() {
            let paddr = page.start_vaddr.as_usize() - axconfig::KERNEL_BASE_VADDR
                + axconfig::KERNEL_BASE_PADDR;
            let _ = memory_set.map_page_without_alloc(
                vdso_base + idx * PAGE_SIZE_4K,
                paddr.into(),
                MappingFlags::READ | MappingFlags::EXECUTE | MappingFlags::USER,
            );
        }
        vdso_base
    }
}
