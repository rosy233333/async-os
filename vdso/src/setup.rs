extern crate xmas_elf;
use crate::{get_vdso_base_end, TaskId};
use core::slice::{from_raw_parts, from_raw_parts_mut};

pub const KERNEL_VDSO_BASE: usize = 0xffff_ffff_c000_0000;
const PAGE_SIZE_4K: usize = 0x1000;

#[link_section = ".data.boot_page_table"]
static mut SECOND_PT_SV39: [u64; 512] = [0; 512];

#[link_section = ".data.boot_page_table"]
static mut THIRD_PT_SV39: [u64; 512] = [0; 512];

/// setup the page table for vDSO
/// the virtual address of vDSO is 0xffff_ffff_c000_0000
/// Safety:
///     这里在初始化启动页表时，需要注意 vDSO 使用的代码段和数据段的大小总和
///     不能超过 512 * 4K = 2M
///     因为这个用的是三级页表，即 4K 来表示的
pub unsafe fn init_vdso_page_table(boot_page_table: *mut [u64; 512]) {
    let (sdata, edata, base, end) = get_vdso_base_end();
    let mut pte_idx = 0;
    for data_base in (sdata..edata).step_by(PAGE_SIZE_4K) {
        THIRD_PT_SV39[pte_idx] = (data_base >> 2) | 0xe7; // VRW__GAD，去掉执行权限
        pte_idx += 1;
    }
    for text_base in (base..end).step_by(PAGE_SIZE_4K) {
        THIRD_PT_SV39[pte_idx] = (text_base >> 2) | 0xef; // VRWX_GAD，这里会写一些全局变量，因此配置了写权限
        pte_idx += 1;
    }
    // setup third page table
    let page_table_4k = &raw const THIRD_PT_SV39 as u64;
    SECOND_PT_SV39[0x0] = (page_table_4k >> 2) | 0x1;
    // setup secondary page table
    let page_table_2m = &raw const SECOND_PT_SV39 as u64;
    (*boot_page_table)[0x1ff] = (page_table_2m >> 2) | 0x1;
    // init_vdso_page_table_second(boot_page_table);
}

/// 这个函数用来测试，内核在初始化 vDSO 时调用
/// setup the page table for vDSO
/// the second virtual address of vDSO is 0xffff_fff0_c000_0000
/// 这个测试需要手动修改一些内容，并且手动进行函数指针的转换
#[allow(unused)]
pub unsafe fn init_vdso_page_table_second(boot_page_table: *mut [u64; 512]) {
    let page_table_2m: u64 = (&raw const SECOND_PT_SV39 as u64);
    (*boot_page_table)[0x1c3] = (page_table_2m >> 2) | 0x1;
}

const fn align_up_64(val: usize) -> usize {
    const SIZE_64BIT: usize = 0x40;
    (val + SIZE_64BIT - 1) & !(SIZE_64BIT - 1)
}

pub fn init_vdso() {
    let (sdata, edata, base, end) = get_vdso_base_end();
    let vdso_text_virt_base = KERNEL_VDSO_BASE + (edata - sdata) as usize;
    let percpu_total_size =
        (percpu::percpu_area_size() * axconfig::SMP + PAGE_SIZE_4K - 1) & !(PAGE_SIZE_4K - 1);
    unsafe {
        from_raw_parts_mut(
            (KERNEL_VDSO_BASE + percpu_total_size) as *mut u8,
            edata as usize - sdata as usize - percpu_total_size,
        )
        .fill(0);
    }
    log::debug!("vdso text base: 0x{:x}", vdso_text_virt_base);
    let elf_data = unsafe { from_raw_parts(base as *const u8, end as usize - base as usize) };

    let elf = xmas_elf::ElfFile::new(&elf_data).expect("Error parsing app ELF file.");
    unsafe { crate::init_vdso_vtable(vdso_text_virt_base as _, &elf) };
    let percpu_size = align_up_64(percpu::percpu_area_size());
    crate::init(percpu_size);
    log::info!("vdso init ok!");
    vdso_test();
}

fn vdso_test() {
    crate::first_add_task(TaskId::new(1, 3, 5));
    crate::first_add_task(TaskId::new(1, 4, 5));
    assert_eq!(TaskId::new(1, 3, 5), crate::pick_next_task());
    assert_eq!(TaskId::new(1, 4, 5), crate::pick_next_task());
    assert_eq!(TaskId::new(0, 0, 0), crate::pick_next_task());
    log::info!("vdso test passed!");
}
