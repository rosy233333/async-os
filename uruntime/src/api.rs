use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;

use crate::current::*;
use crate::task::TaskInner;
use crate::Scheduler;
use crate::Task;
use crate::TaskRef;

// Initializes the executor (for the primary CPU).
pub fn init() {
    println!("init uruntime");
    let mut scheduler = Scheduler::new();
    scheduler.init();
    SCHEDULER.with(|s| s.replace(Arc::new(Mutex::new(scheduler))));
    println!("  use {} scheduler.", Scheduler::scheduler_name());
}

// #[cfg(feature = "smp")]
// /// Initializes the executor for secondary CPUs.
// pub fn init_secondary() {
//     assert!(KERNEL_EXECUTOR.is_init());
//     taskctx::init();
//     let kexecutor = KERNEL_EXECUTOR.clone();
//     unsafe { CurrentExecutor::init_current(kexecutor) };
// }

pub fn current_task_may_uninit() -> Option<CurrentTask> {
    CurrentTask::try_get()
}

pub fn current_task() -> CurrentTask {
    CurrentTask::get()
}

/// Spawns a new task with the given parameters.
///
/// Returns the task reference.
pub fn spawn_raw<F, T>(f: F, name: String) -> TaskRef
where
    F: FnOnce() -> T,
    T: Future<Output = isize> + 'static,
{
    let scheduler = SCHEDULER.with(|s| s.borrow().clone());
    let task = Arc::new(Task::new(TaskInner::new(
        name,
        scheduler.clone(),
        Box::pin(f()),
    )));
    scheduler.lock().unwrap().add_task(task.clone());
    task
}

pub fn pick_next_task() -> Option<TaskRef> {
    SCHEDULER.with(|s| s.borrow().lock().unwrap().pick_next_task())
}

pub fn put_prev_task(task: TaskRef) {
    SCHEDULER.with(|s| s.borrow().lock().unwrap().put_prev_task(task, false))
}
