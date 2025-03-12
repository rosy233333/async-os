//! 这里用于内核初始化 vDSO

#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use async_mem::MemorySet;
use axalloc::PhysPage;
use axhal::paging::MappingFlags;
use core::{cell::UnsafeCell, ptr::{copy_nonoverlapping, slice_from_raw_parts}};
use elf_parser::get_relocate_pairs;
use lazy_init::LazyInit;
use memory_addr::{VirtAddr, PAGE_SIZE_4K};

mod prio_queue;
mod data;
pub use data::*;

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
// #[link_section = ".vvar"]
pub static VDSO_DATA: VdsoData = VdsoData::new(); // VVAR数据区域
// #[link_section = ".vvar"]
// pub static VDSO_DATA: [u8; VVAR_SIZE] = [0; VVAR_SIZE]; // VVAR数据区域

pub struct VdsoInfo {
    pub name: &'static str,
    pub elf_data: &'static [u8],
    pub vvar_data: &'static [u8],
    pub cm: Vec<PhysPage>,
    pub dm: Vec<PhysPage>,
}

impl VdsoInfo {
    // 需要提供内核MemorySet，从而为vdso代码和数据重新分配空间。
    pub fn new() -> Self {
        // 初始化代码区域
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

        // 初始化数据区域
        // let vdso_data = VdsoData::new();
        // // SAFETY: 因为位于不同的段中，因此一定是不重叠的
        // unsafe { copy_nonoverlapping(&vdso_data as *const VdsoData, &VDSO_DATA as *const [u8; VVAR_SIZE] as *const () as *mut () as *mut VdsoData, 1) };
        let dm = (0..VVAR_PAGES)
        .map(|i| PhysPage {
            start_vaddr: (&VDSO_DATA as *const VdsoData as usize + i * PAGE_SIZE_4K).into(),
        })
        .collect::<Vec<PhysPage>>();
        let vvar_data: &[u8] = unsafe { &*slice_from_raw_parts(&VDSO_DATA as *const VdsoData as *const () as *const u8, size_of::<VdsoData>()) };

        // // start是数据原本的起始地址，base是重分配后的起始地址
        // let vvar_len: usize = size_of<VdsoData>();// 未对齐到页，只是数据结构的大小
        // let vvar_start: *const u8 = &VDSO_DATA as *const VdsoData as *const u8;
        // let vvar: &[u8] = unsafe {&*slice_from_raw_parts(vvar_start, vvar_len)};
        // let vvar_base: usize = memory_set.max_va();
        // let vdso_len: usize = vdso_end as usize - vdso_start as usize; // 已对齐到页 
        // let vdso_start: *const u8 = vdso_start as usize as *const u8;
        // let vdso: &[u8] = unsafe {&*slice_from_raw_parts(vdso_start, vdso_len)};
        // let vdso_base: usize = vvar_base + VVAR_SIZE;
        // assert_eq!(&vdso[0..4], b"\x7fELF");

        // // 为代码区域和数据区域重新分配空间，以保证代码区域紧邻数据区域之后
        // memory_set.new_region(
        //     vvar_base,
        //     VVAR_SIZE,
        //     false,
        //     MappingFlags::READ | MappingFlags::WRITE,
        //     Some(vvar),
        //     None
        // ).await;
        
        // memory_set.new_region(
        //     vdso_base,
        //     vdso_len,
        //     false,
        //     MappingFlags::READ | MappingFlags::EXECUTE,
        //     Some(vdso),
        //     None
        // ).await;

        // let elf_data: &[u8] = unsafe { &*slice_from_raw_parts(vdso_base as *const u8, vdso_len) };
        // let vvar_data: &[u8] = unsafe { &*slice_from_raw_parts(vvar_base as *const u8, vvar_len) };

        // let cm: Vec<PhysPage> = (0 .. (vdso_len / PAGE_SIZE_4K))
        //     .map(|i| PhysPage {
        //         start_vaddr: (vdso_base as usize + i * PAGE_SIZE_4K).into(),
        //     })
        //     .collect::<Vec<PhysPage>>();
        // let dm = (0 .. VVAR_PAGES)
        //     .map(|i| PhysPage {
        //         start_vaddr: (vvar_base as usize + i * PAGE_SIZE_4K).into(),
        //     })
        //     .collect::<Vec<PhysPage>>();

        Self {
            name: "vdso",
            elf_data,
            vvar_data,
            cm,
            dm
        }
    }

    pub async fn vdso2memoryset(&self, memory_set: &mut MemorySet) -> VirtAddr {
        let vvar_base = memory_set.max_va();
        let vdso_base = vvar_base + VVAR_SIZE;
        log::warn!("vvar_base: {:x}, vvar_size: {:x}", vvar_base.as_usize(), VVAR_SIZE);
        log::warn!("vdso_base: {:x}, vdso_size: {:x}", vdso_base.as_usize(), vdso_end as usize - vdso_start as usize);

        // 映射 vDSO 代码区域
        let vdso_paddr = self.cm[0].start_vaddr.as_usize() - axconfig::KERNEL_BASE_VADDR
            + axconfig::KERNEL_BASE_PADDR;
        let _ = memory_set
            .map_attach_page_without_alloc(
                vdso_base,
                vdso_paddr.into(),
                self.cm.len(),
                MappingFlags::READ | MappingFlags::EXECUTE | MappingFlags::USER,
            )
            .await;

        // 映射 vdso 数据区域
        let vvar_paddr = self.dm[0].start_vaddr.as_usize() - axconfig::KERNEL_BASE_VADDR
            + axconfig::KERNEL_BASE_PADDR;
        let _ = memory_set
        .map_attach_page_without_alloc(
            vvar_base,
            vvar_paddr.into(),
            self.dm.len(),
            MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
        )
        .await;
        // let _ = memory_set
        //     .new_region(
        //         vvar_base,
        //         VVAR_SIZE,
        //         false,
        //         MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
        //         Some(&[0; VVAR_SIZE]),
        //         // None,
        //         None,
        //     )
        //     .await;

        let elf = xmas_elf::ElfFile::new(self.elf_data).expect("Error parsing vDSO.");
        let relocate_pairs = get_relocate_pairs(&elf, Some(vdso_base.as_usize()));
        for relocate_pair in relocate_pairs {
            let src_va: usize = relocate_pair.src.into();
            let dst_va: usize = relocate_pair.dst.into();
            let src_pa: usize = memory_set.query(relocate_pair.src).unwrap().0.into();
            let dst_pa: usize = memory_set.query(relocate_pair.dst).unwrap().0.into();
            // let src_pa: usize = src_va;
            // let dst_pa: usize = dst_va;
            let count = relocate_pair.count;
            log::error!("src_va: {:#x}, dst_va: {:#x}, src_pa: {:#x}, dst_pa: {:#x}, count: {:#x}", src_va, dst_va, src_pa, dst_pa, count);
            unsafe { copy_nonoverlapping(src_pa.to_ne_bytes().as_ptr(), dst_pa as *mut u8, count) }
        }

        vdso_base
    }
}
