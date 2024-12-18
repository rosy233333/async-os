#![no_std]
#![feature(naked_functions)]

extern crate alloc;
extern crate log;

mod arch;
mod current;
mod kstack;
mod stat;
mod task;
mod waker;

use alloc::sync::Arc;
pub use arch::TrapFrame;
pub use arch::TrapStatus;
pub use current::CurrentTask;
pub use kstack::init;
pub use kstack::TaskStack;
pub use waker::waker_from_task;

pub type TaskRef = Arc<Task>;
pub use kstack::*;
pub use scheduler::BaseScheduler;
pub use task::{SchedPolicy, SchedStatus, TaskId, TaskInner, TaskState};

#[cfg(feature = "thread")]
pub use task::{CtxType, StackCtx};

cfg_if::cfg_if! {
    if #[cfg(feature = "sched_rr")] {
        const MAX_TIME_SLICE: usize = 5;
        pub type Task = scheduler::RRTask<TaskInner, MAX_TIME_SLICE>;
        pub type Scheduler = scheduler::RRScheduler<TaskInner, MAX_TIME_SLICE>;
    } else if #[cfg(feature = "sched_cfs")] {
        pub type Task = scheduler::CFSTask<TaskInner>;
        pub type Scheduler = scheduler::CFScheduler<TaskInner>;
    } else {
        // If no scheduler features are set, use FIFO as the default.
        pub type Task = scheduler::FifoTask<TaskInner>;
        pub type Scheduler = scheduler::FifoScheduler<TaskInner>;
    }
}

/// 这里不对任务的状态进行修改，在调用 waker.wake() 之前对任务状态进行修改
pub fn wakeup_task(task: TaskRef) {
    task.set_state(TaskState::Runable);
    // log::debug!("wakeup task {}, count {}", task.id_name(), Arc::strong_count(&task));
    // #[cfg(feature = "preempt")]
    // let preempt = task.get_preempt_pending();
    // #[cfg(not(feature = "preempt"))]
    // let preempt = false;
    // task.clone().scheduler.lock().lock().put_prev_task(task, preempt);
    task.clone().scheduler.lock().lock().add_task(task);

}

#[cfg(feature = "preempt")]
use kernel_guard::KernelGuardIf;

#[cfg(feature = "preempt")]
struct KernelGuardIfImpl;

#[cfg(feature = "preempt")]
#[crate_interface::impl_interface]
impl KernelGuardIf for KernelGuardIfImpl {
    fn enable_preempt() {
        // Your implementation here
        if let Some(curr) = CurrentTask::try_get() {
            curr.enable_preempt();
        }
    }
    fn disable_preempt() {
        // Your implementation here
        if let Some(curr) = CurrentTask::try_get() {
            curr.disable_preempt();
        }
    }
}
