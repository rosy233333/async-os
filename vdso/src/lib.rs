//! 这里用于内核初始化 vDSO

#![no_std]

extern crate alloc;

use alloc::{collections::btree_set::Union, vec::Vec};
use async_mem::MemorySet;
use axalloc::PhysPage;
use axhal::paging::MappingFlags;
use core::{cell::UnsafeCell, ptr::copy_nonoverlapping};
use elf_parser::get_relocate_pairs;
use lazy_init::LazyInit;
use log::{info, warn};
use memory_addr::{VirtAddr, PAGE_SIZE_4K};

// core::arch::global_asm!(include_str!("vdso.S"));
static SO_CONTENT: &[u8] = include_bytes!("../libvdsoexample.so");
static VDSO_SIZE: usize = ((SO_CONTENT.len() - 1) / PAGE_SIZE_4K + 1) * PAGE_SIZE_4K;

pub fn init() {
    VDSO_INFO.init_by(VdsoInfo::new());
    VDSO_INFO.test_vdso();
}

// extern "C" {
//     fn VDSO_START();
//     fn VDSO_END();
// }

static VVAR_PAGES: usize = 1;
static VVAR_SIZE: usize = VVAR_PAGES * PAGE_SIZE_4K;

struct SyncUnsafeCell<T>(UnsafeCell<T>);
unsafe impl<T> Sync for SyncUnsafeCell<T> {}

#[link_section = ".vvar"]
#[no_mangle]
static VVAR: SyncUnsafeCell<[u8; VVAR_SIZE]> = SyncUnsafeCell(UnsafeCell::new([0; VVAR_SIZE]));
#[link_section = ".vdso"]
#[no_mangle]
static VDSO: SyncUnsafeCell<[u8; VDSO_SIZE]> = SyncUnsafeCell(UnsafeCell::new([0; VDSO_SIZE]));

// #[link_section = "vvar"]
// static VVAR: [u8; VVAR_SIZE] = [0; VVAR_SIZE];
// #[link_section = "vdso"]
// static VDSO: [u8; VDSO_SIZE] = [0; VDSO_SIZE];

pub static VDSO_INFO: LazyInit<VdsoInfo> = LazyInit::new();

pub struct VdsoInfo {
    pub name: &'static str,
    pub elf_data: &'static [u8],
    pub cm: Vec<PhysPage>,
}

impl VdsoInfo {
    pub fn new() -> Self {
        info!("Initialize vDSO...");
        unsafe {
            (&mut *VVAR.0.get())[0..4].copy_from_slice(&[0xde, 0xad, 0xbe, 0xef]);
        }
        unsafe {
            (&mut *VDSO.0.get())[0..SO_CONTENT.len()].copy_from_slice(SO_CONTENT);
        }
        // unsafe {
        //     (&mut *(&VDSO as *const [u8; VDSO_SIZE] as *mut [u8; VDSO_SIZE]))[0..SO_CONTENT.len()]
        //         .copy_from_slice(SO_CONTENT);
        // }

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
            // unsafe { copy_nonoverlapping(src.to_ne_bytes().as_ptr(), dst as *mut u8, count) }
        }
        // 映射 vDSO 代码区域
        let paddr = self.cm[0].start_vaddr.as_usize() - axconfig::KERNEL_BASE_VADDR
            + axconfig::KERNEL_BASE_PADDR;
        let _ = memory_set
            .map_attach_page_without_alloc(
                vdso_base,
                paddr.into(),
                self.cm.len(),
                MappingFlags::READ | MappingFlags::EXECUTE | MappingFlags::USER,
            )
            .await;
        vdso_base
    }

    pub fn test_vdso(&self) {
        warn!("Testing vDSO in kernel...");
        api::init();
        assert!(api::api_example().i == 42);
        warn!("Test passed!");
    }
}
