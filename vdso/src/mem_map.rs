/// 添加 vDSO 相关的映射
pub fn add_kernel_vdso_mapping(kernel_page_table: &mut axhal::paging::PageTable) {
    let (vdso_sdata, vdso_edata, vdso_base, vdso_end) = crate::get_vdso_base_end();
    let vdso_sdata_phy = axhal::mem::virt_to_phys((vdso_sdata as usize).into());
    kernel_page_table
        .map_region(
            axhal::mem::VirtAddr::from(crate::KERNEL_VDSO_BASE),
            vdso_sdata_phy,
            (vdso_edata - vdso_sdata) as usize,
            axhal::mem::MemRegionFlags::from_bits(1 << 0 | 1 << 1 | 1 << 6)
                .unwrap()
                .into(),
            true,
        )
        .unwrap();
    let vdso_base_phy = axhal::mem::virt_to_phys((vdso_base as usize).into());
    kernel_page_table
        .map_region(
            axhal::mem::VirtAddr::from(
                crate::KERNEL_VDSO_BASE + (vdso_edata - vdso_sdata) as usize,
            ),
            vdso_base_phy,
            (vdso_end - vdso_base) as usize,
            axhal::mem::MemRegionFlags::from_bits(1 << 0 | 1 << 1 | 1 << 2 | 1 << 6)
                .unwrap()
                .into(),
            true,
        )
        .unwrap();
}

extern "C" {
    fn _percpu_start();
    fn _percpu_end();
}

pub fn vdso_percpu_map(page_table: &mut axhal::paging::PageTable) {
    add_kernel_vdso_mapping(page_table);
    // vdso 中的映射只添加了另一个虚拟地址对 percpu 段的访问，
    // 这里还需要按照原本的方式来建立线性地址映射

    let percpu_size = _percpu_end as usize - _percpu_start as usize;
    page_table
        .map_region(
            (_percpu_start as usize).into(),
            axhal::mem::virt_to_phys((_percpu_start as usize).into()),
            percpu_size,
            axhal::mem::MemRegionFlags::from_bits(1 << 0 | 1 << 1 | 1 << 2 | 1 << 6)
                .unwrap()
                .into(),
            true,
        )
        .expect("Error mapping kernel memory");
}
