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
