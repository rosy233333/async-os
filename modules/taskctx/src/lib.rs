#![no_std]
#![feature(asm_const)]
#![feature(naked_functions)]

extern crate alloc;
extern crate log;

mod arch;
mod task;
mod kstack;
mod current;
mod waker;
mod stat;

use alloc::sync::Arc;
pub use arch::TrapFrame;
pub use arch::TrapStatus;
pub use waker::waker_from_task;
pub use current::CurrentTask;
pub use kstack::TaskStack;
pub use kstack::init;

pub type TaskRef = Arc<Task>;
pub use task::{TaskInner, TaskId, TaskState, SchedPolicy, SchedStatus};
pub use scheduler::BaseScheduler;
pub use kstack::*;

#[cfg(feature = "thread")]
pub use task::{StackCtx, CtxType};

cfg_if::cfg_if! {
    if #[cfg(feature = "sched_rr")] {
        const MAX_TIME_SLICE: usize = 5;
        pub type Task = scheduler::RRTask<TaskInner, MAX_TIME_SLICE>;
        pub type Scheduler = scheduler::RRScheduler<TaskInner, MAX_TIME_SLICE>;
    } else if #[cfg(feature = "sched_cfs")] {
        pub type Task = scheduler::CFSTask<TaskInner>;
        pub type Scheduler = scheduler::CFScheduler<TaskInner>;
    } else if #[cfg(feature = "sched_moic")] {
        pub type Task = scheduler::MOICTask<TaskInner>;
        pub type Scheduler = scheduler::MOICScheduler<TaskInner>;
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
    task.clone()
        .scheduler.lock()
        .lock()
        .put_prev_task(task, false);
}
