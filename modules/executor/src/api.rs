use crate::{
    flags::WaitStatus, futex::futex_wake, send_signal_to_process, send_signal_to_thread, CurrentExecutor, Executor, KERNEL_EXECUTOR, KERNEL_EXECUTOR_ID, PID2PC, TID2TASK, UTRAP_HANDLER
};
use alloc::{boxed::Box, string::String, sync::Arc};
use axsignal::signal_no::SignalNo;
use core::{future::Future, ops::Deref, pin::Pin};
pub use task_api::*;

// Initializes the executor (for the primary CPU).
pub fn init(utrap_handler: fn() -> Pin<Box<dyn Future<Output = isize> + 'static>>) {
    info!("Initialize executor...");
    taskctx::init();
    UTRAP_HANDLER.init_by(utrap_handler);
    let kexecutor = Arc::new(Executor::new_init());
    KERNEL_EXECUTOR.init_by(kexecutor.clone());
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
    T: Future<Output = isize> + 'static,
{
    let scheduler = current_executor().get_scheduler();
    let task = Arc::new(Task::new(TaskInner::new(
        name,
        KERNEL_EXECUTOR_ID,
        scheduler.clone(),
        0,
        Box::pin(f()),
    )));
    scheduler.lock().add_task(task.clone());
    task
}

pub async fn exit(exit_code: isize) {
    let curr = current_task();
    let curr_id = curr.id().as_u64();

    let current_executor = current_executor();
    info!("exit task id {} with code _{}_", curr_id, exit_code);

    let exit_signal = current_executor
        .signal_modules
        .lock()
        .await
        .get(&curr_id)
        .unwrap()
        .get_exit_signal();

    if exit_signal.is_some() || curr.is_leader() {
        let parent = current_executor.get_parent();
        if parent != KERNEL_EXECUTOR_ID {
            // send exit signal
            let signal = if exit_signal.is_some() {
                exit_signal.unwrap()
            } else {
                SignalNo::SIGCHLD
            };
            send_signal_to_process(parent as isize, signal as isize, None)
                .await
                .unwrap();
        }
    }

    // clear_child_tid 的值不为 0，则将这个用户地址处的值写为0
    let clear_child_tid = curr.get_clear_child_tid();
    if clear_child_tid != 0 {
        // 先确认是否在用户空间
        if current_executor
            .manual_alloc_for_lazy(clear_child_tid.into())
            .await
            .is_ok()
        {
            unsafe {
                *(clear_child_tid as *mut i32) = 0;
                // TODO:
                let _ = futex_wake(clear_child_tid.into(), 0, 1).await;
            }
        }
    }
    if curr.is_leader() {
        loop {
            let mut all_exited = true;
            // TODO：这里是存在问题的，处于 blocked 状态的任务是无法收到信号的
            while let Some(task) = current_executor.get_scheduler().lock().pick_next_task() {
                if !task.is_leader() && task.state() != TaskState::Exited {
                    all_exited = false;
                    send_signal_to_thread(task.id().as_u64() as isize, SignalNo::SIGKILL as isize)
                        .await
                        .unwrap();
                }
            }
            if !all_exited {
                yield_now().await;
            } else {
                break;
            }
        }
        TID2TASK.lock().await.remove(&curr_id);
        curr.set_exit_code(exit_code);
        curr.set_state(TaskState::Exited);
        current_executor.set_exit_code(exit_code);
        current_executor.set_zombie(true);
        current_executor.exit_main_task().await;
        while let Some(task) = current_executor.get_scheduler().lock().pick_next_task() {
            drop(task);
        }

        current_executor.fd_manager.fd_table.lock().await.clear();

        current_executor.signal_modules.lock().await.clear();

        let mut pid2pc = PID2PC.lock().await;
        let kernel_executor = &*KERNEL_EXECUTOR;
        // 将子进程交给idle进程
        // process.memory_set = Arc::clone(&kernel_process.memory_set);
        for child in current_executor.children.lock().await.deref() {
            child.set_parent(KERNEL_EXECUTOR_ID);
            kernel_executor
                .children
                .lock()
                .await
                .push(Arc::clone(child));
        }
        if let Some(parent_process) = pid2pc.get(&current_executor.get_parent()) {
            parent_process.set_vfork_block(false).await;
        }
        pid2pc.remove(&current_executor.pid());
        drop(pid2pc);
        drop(current_executor);
    } else {
        TID2TASK.lock().await.remove(&curr_id);
        curr.set_exit_code(exit_code);
        curr.set_state(TaskState::Exited);
        // 从进程中删除当前线程
        current_executor
            .signal_modules
            .lock()
            .await
            .remove(&curr_id);
    }
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
    T: Future<Output = isize> + 'static,
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
                info!(
                    "wait pid _{}_ with code _{}_",
                    child.pid(),
                    exit_code
                );
                exit_task_id = index;
                if !exit_code_ptr.is_null() {
                    unsafe {
                        // 因为没有切换页表，所以可以直接填写
                        *exit_code_ptr = (exit_code as i32) << 8;
                    }
                }
                answer_id = child.pid();
                break;
            }
        } else if child.pid() == pid as u64 {
            // 找到了对应的进程
            if let Some(exit_code) = child.get_code_if_exit() {
                answer_status = WaitStatus::Exited;
                info!(
                    "wait pid _{}_ with code _{:?}_",
                    child.pid(),
                    exit_code
                );
                exit_task_id = index;
                if !exit_code_ptr.is_null() {
                    unsafe {
                        *exit_code_ptr = (exit_code as i32) << 8;
                        // 用于WEXITSTATUS设置编码
                    }
                }
                answer_id = child.pid();
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
