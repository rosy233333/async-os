use alloc::sync::Arc;

use crate::SyscallResult;
use axconfig::{MMIO_REGIONS, PHYS_VIRT_OFFSET};
use axhal::paging::MappingFlags;
use executor::current_executor;
const UTAIC_BASE: usize = 0xffff_0000;
use taic_driver::{Taic, TaskMeta};

pub async fn syscall_get_taic() -> SyscallResult {
    let paddr = MMIO_REGIONS[1].0;
    let current_executor = current_executor().await;
    let taic = Taic::new(PHYS_VIRT_OFFSET + paddr);
    let os_id = Arc::new(TaskMeta::<usize>::init()).into();
    taic.switch_os::<usize>(Some(os_id));
    let process_id = Arc::new(TaskMeta::<usize>::init()).into();
    taic.switch_process::<usize>(Some(process_id));

    let _ = current_executor
        .memory_set
        .lock()
        .await
        .map_page_without_alloc(
            UTAIC_BASE.into(),
            paddr.into(),
            MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
        );
    Ok(UTAIC_BASE as isize)
}
