use alloc::boxed::Box;
use core::cell::Cell;
use core::future::Future;
use core::task::Context;
use core::task::Poll;
use syscalls::TaskOps;

// 供syscalls库使用的任务接口
struct TaskOpsImpl;

#[crate_interface::impl_interface]
impl TaskOps for TaskOpsImpl {
    #[cfg(feature = "thread")]
    fn yield_now() {
        task_management::yield_current_to_local();
    }

    #[cfg(not(feature = "thread"))]
    fn set_state_yield() {
        task_management::set_current_state_yield();
    }
}
