extern crate alloc;

mod api;
mod current_scheduler;
use std::{sync::Arc, task::Context};
// #[allow(unused)]
// mod scheduler;

pub use api::*;
pub use utaskctx::CurrentTask;
use scheduler::*;
use spinlock::SpinRaw;
use utaskctx::*;

#[cfg(feature = "shared_scheduler")]
pub type Task = FifoTask<TaskInner>;
#[cfg(not(feature = "shared_scheduler"))]
pub type Task = TAICTask<TaskInner>;
#[cfg(feature = "shared_scheduler")]
pub type Scheduler = FifoScheduler<TaskInner>;
#[cfg(not(feature = "shared_scheduler"))]
pub type Scheduler = TAICScheduler<TaskInner>;
pub type TaskRef = Arc<Task>;

pub async fn uprocess_ktask_ucontrolflow() {
    loop {
        while let Some(task) = pick_next_task() {
            println!("run: {}", task.id_name());
            CurrentTask::init_current(task);
            let curr = current_task();
            let waker = curr.waker();
            let mut cx = Context::from_waker(&waker);
            match curr.get_fut().as_mut().poll(&mut cx) {
                std::task::Poll::Ready(exit_code) => {
                    println!("task is ready: {}", exit_code);
                    CurrentTask::clean_current();
                }
                std::task::Poll::Pending => {
                    println!("task is pending");
                    CurrentTask::clean_current_without_drop();
                }
            }
        }
    }
}
