//! Define the trap handler for the whole kernel
pub use axhal::{mem::VirtAddr, paging::MappingFlags, time::current_time_nanos};
use axsignal::signal_no::SignalNo;
use process::{current_process, current_task, send_signal_to_thread};

use super::syscall::syscall;

fn time_stat_from_kernel_to_user() {
    current_task().time_stat_from_kernel_to_user(current_time_nanos() as usize);
}

fn time_stat_from_user_to_kernel() {
    current_task().time_stat_from_user_to_kernel(current_time_nanos() as usize);
}

/// Handle the interrupt
///
/// # Arguments
///
/// * `irq_num` - The number of the interrupt
///
/// * `from_user` - Whether the interrupt is from user space
pub fn handle_irq(_irq_num: usize, from_user: bool) {
    // trap进来，统计时间信息
    // 只有当trap是来自用户态才进行统计
    if from_user {
        time_stat_from_user_to_kernel();
    }
    #[cfg(feature = "irq")]
    async_axhal::irq::dispatch_irq(_irq_num);
    if from_user {
        time_stat_from_kernel_to_user();
    }
}

/// Handle the syscall
///
/// # Arguments
///
/// * `syscall_id` - The id of the syscall
///
/// * `args` - The arguments of the syscall
pub async fn handle_syscall(syscall_id: usize, args: [usize; 6]) -> isize {
    time_stat_from_user_to_kernel();
    let ans = syscall(syscall_id, args).await;
    time_stat_from_kernel_to_user();
    ans
}

/// Handle the page fault exception
///
/// # Arguments
///
/// * `addr` - The address where the page fault occurs
///
/// * `flags` - The permission which the page fault needs
pub async fn handle_page_fault(addr: VirtAddr, flags: MappingFlags) {
    time_stat_from_user_to_kernel();
    let current_process = current_process().await;
    if current_process
        .memory_set
        .lock()
        .await
        .handle_page_fault(addr, flags)
        .await
        .is_ok()
    {
        axhal::arch::flush_tlb(None);
    } else {
        let _ = send_signal_to_thread(
            current_task().id().as_u64() as isize,
            SignalNo::SIGSEGV as isize,
        )
        .await;
    }
    time_stat_from_kernel_to_user();
}

/// To handle the pending signals for current process
pub async fn handle_signals() {
    time_stat_from_user_to_kernel();
    process::signal::handle_signals().await;
    time_stat_from_kernel_to_user();
}
