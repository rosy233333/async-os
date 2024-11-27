use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;

use syscalls::raw::*;

use crate::current::*;
use crate::task::TaskInner;
use crate::Scheduler;
use crate::Task;
use crate::TaskRef;
use taic_driver::TaskId;

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

/// 需要由 dispatcher 来进行初始化并行批处理异步系统调用
pub fn init_batch_async_syscall() -> AsyncBatchSyscallCfg {
    const INIT_BATCH_ASYNC: usize = 556;
    let res = AsyncBatchSyscallCfg::default();
    let _ = unsafe {
        syscall2(
            INIT_BATCH_ASYNC,
            current_task().waker().data() as _,
            &res as *const _ as usize,
            None,
        )
    };
    res
}

pub fn issue_syscall(cfg: &AsyncBatchSyscallCfg) {
    let send_queue = unsafe { &mut *(cfg.recv_channel as *mut SyscallItemQueue) };
    let _ = send_queue.enqueue(SyscallItem {
        id: 0,
        args: [0x19990109; 6],
        ret_ptr: 0x19990109,
        waker: 0x19990109,
    });
    SCHEDULER.with(|s| unsafe {
        s.borrow().lock().unwrap().send(
            TaskId::virt(cfg.recv_os_id),
            TaskId::virt(cfg.recv_process_id),
            TaskId::virt(cfg.recv_task_id),
        )
    })
}

#[allow(unused)]
#[derive(Default, Debug)]
pub struct AsyncBatchSyscallCfg {
    pub send_channel: usize,
    pub recv_channel: usize,
    pub recv_os_id: usize,
    pub recv_process_id: usize,
    pub recv_task_id: usize,
}

use heapless::mpmc::MpMcQueue;
type SyscallItemQueue = MpMcQueue<SyscallItem, 8>;

#[repr(C, align(128))]
#[derive(Debug)]
struct SyscallItem {
    id: usize,
    args: [usize; 6],
    ret_ptr: usize,
    waker: usize,
}
