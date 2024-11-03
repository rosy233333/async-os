use crate::{
    flags::WaitStatus, CurrentExecutor, Executor, 
    EXECUTORS, KERNEL_EXECUTOR, KERNEL_EXECUTOR_ID, TID2TASK, UTRAP_HANDLER
};
use core::{future::Future, pin::Pin};
use alloc::{boxed::Box, string::String, sync::Arc};
pub use task_api::*;

// Initializes the executor (for the primary CPU).
pub fn init(utrap_handler: fn() -> Pin<Box<dyn Future<Output = i32> + 'static>>) {
    info!("Initialize executor...");
    taskctx::init();
    UTRAP_HANDLER.init_by(utrap_handler);
    let kexecutor = Arc::new(Executor::new_init());
    KERNEL_EXECUTOR.init_by(kexecutor.clone());
    EXECUTORS.lock().insert(0, kexecutor.clone());
    unsafe { CurrentExecutor::init_current(kexecutor) };
    #[cfg(feature = "irq")]
    task_api::init();
    info!("  use {} scheduler.", Scheduler::scheduler_name());
}

#[cfg(feature = "smp")]
/// Initializes the executor for secondary CPUs.
pub fn init_secondary() {
    assert!(KERNEL_EXECUTOR.is_init());
    taskctx::init();
    let kexecutor = KERNEL_EXECUTOR.clone();
    unsafe { CurrentExecutor::init_current(kexecutor) };
}

pub fn current_task_may_uninit() -> Option<CurrentTask> {
    CurrentTask::try_get()
}

pub fn current_task() -> CurrentTask {
    CurrentTask::get()
}

pub fn current_executor() -> CurrentExecutor {
    CurrentExecutor::get()
}

/// Spawns a new task with the given parameters.
/// 
/// Returns the task reference.
pub fn spawn_raw<F, T>(f: F, name: String) -> TaskRef
where
    F: FnOnce() -> T,
    T: Future<Output = i32> + 'static,
{
    let scheduler = current_executor().get_scheduler();
    let task = Arc::new(Task::new(
        TaskInner::new(name, KERNEL_EXECUTOR_ID, scheduler.clone(), Box::pin(f()))
    ));
    scheduler.lock().add_task(task.clone());    
    task
}

pub async fn exit(exit_code: i32) {
    let curr = current_task();
    TID2TASK.lock().await.remove(&curr.id().as_u64());
    curr.set_exit_code(exit_code);
    curr.set_state(TaskState::Exited);
    let current_executor = current_executor();
    current_executor.exit_main_task().await;
    current_executor.set_exit_code(exit_code);
    current_executor.set_zombie(true);
}

/// Spawns a new task with the default parameters.
/// 
/// The default task name is an empty string. The default task stack size is
/// [`axconfig::TASK_STACK_SIZE`].
/// 
/// Returns the task reference.
pub fn spawn<F, T>(f: F) -> TaskRef
where
    F: FnOnce() -> T,
    T: Future<Output = i32> + 'static,
{
    spawn_raw(f, "".into())
}

/// Set the priority for current task.
///
/// The range of the priority is dependent on the underlying scheduler. For
/// example, in the [CFS] scheduler, the priority is the nice value, ranging from
/// -20 to 19.
///
/// Returns `true` if the priority is set successfully.
///
/// [CFS]: https://en.wikipedia.org/wiki/Completely_Fair_Scheduler
pub fn set_priority(prio: isize) -> bool {
    current_executor().set_priority(current_task().as_task_ref(), prio)
}


/// 在当前进程找对应的子进程，并等待子进程结束
/// 若找到了则返回对应的pid
/// 否则返回一个状态
///
/// # Safety
///
/// 保证传入的 ptr 是有效的
pub async unsafe fn wait_pid(pid: i32, exit_code_ptr: *mut i32) -> Result<u64, WaitStatus> {
    // 获取当前进程
    let curr_process = current_executor();
    let mut exit_task_id: usize = 0;
    let mut answer_id: u64 = 0;
    let mut answer_status = WaitStatus::NotExist;
    for (index, child) in curr_process.children.lock().await.iter().enumerate() {
        if pid <= 0 {
            if pid == 0 {
                axlog::warn!("Don't support for process group.");
            }
            // 任意一个进程结束都可以的
            answer_status = WaitStatus::Running;
            if let Some(exit_code) = child.get_code_if_exit() {
                answer_status = WaitStatus::Exited;
                info!("wait pid _{}_ with code _{}_", child.pid().as_u64(), exit_code);
                exit_task_id = index;
                if !exit_code_ptr.is_null() {
                    unsafe {
                        // 因为没有切换页表，所以可以直接填写
                        *exit_code_ptr = exit_code << 8;
                    }
                }
                answer_id = child.pid().as_u64();
                break;
            }
        } else if child.pid().as_u64() == pid as u64 {
            // 找到了对应的进程
            if let Some(exit_code) = child.get_code_if_exit() {
                answer_status = WaitStatus::Exited;
                info!("wait pid _{}_ with code _{:?}_", child.pid().as_u64(), exit_code);
                exit_task_id = index;
                if !exit_code_ptr.is_null() {
                    unsafe {
                        *exit_code_ptr = exit_code << 8;
                        // 用于WEXITSTATUS设置编码
                    }
                }
                answer_id = child.pid().as_u64();
            } else {
                answer_status = WaitStatus::Running;
            }
            break;
        }
    }
    // 若进程成功结束，需要将其从父进程的children中删除
    if answer_status == WaitStatus::Exited {
        curr_process.children.lock().await.remove(exit_task_id);
        return Ok(answer_id);
    }
    Err(answer_status)
}