use crate::{
    flags::WaitStatus, futex::futex_wake, send_signal_to_process, send_signal_to_thread, Process,
    KERNEL_PROCESS_ID, PID2PC, TID2TASK, UTRAP_HANDLER,
};
use alloc::{boxed::Box, string::String, sync::Arc};
use axsignal::signal_no::SignalNo;
use core::{future::Future, ops::Deref, pin::Pin};
pub use task_api::*;

// Initializes the process (for the primary CPU).
pub fn init(utrap_handler: fn() -> Pin<Box<dyn Future<Output = isize> + 'static>>) {
    info!("Initialize process...");
    UTRAP_HANDLER.init_by(utrap_handler);
}

#[cfg(feature = "smp")]
/// Initializes the process for secondary CPUs.
pub fn init_secondary() {}

pub fn current_task_may_uninit() -> Option<CurrentTask> {
    CurrentTask::try_get()
}

pub fn current_task() -> CurrentTask {
    CurrentTask::get()
}

pub async fn current_process() -> Arc<Process> {
    let current_task = current_task();
    let current_process = Arc::clone(
        PID2PC
            .lock()
            .await
            .get(&current_task.get_process_id())
            .unwrap(),
    );
    current_process
}

/// Spawns a new task with the given parameters.
///
/// Returns the task reference.
pub fn spawn_raw<F, T>(f: F, name: String) -> TaskRef
where
    F: FnOnce() -> T,
    T: Future<Output = isize> + 'static,
{
    let scheduler = current_scheduler().clone();
    let task = Arc::new(Task::new(TaskInner::new(
        name,
        KERNEL_PROCESS_ID,
        scheduler.clone(),
        0,
        Box::pin(f()),
    )));
    scheduler.lock().add_task(task.clone());
    task
}

// 这里直接将进程从 PID2PC 中删除会导致问题
// 因为当两个核并行时，这里的还没有回到调度函数，
// 其它核上运行的父进程 wait 到这个任务后，将会把进程释放掉，页表也会失效
// 这个核此时才回到调度函数，就会产生页错误
// 1. 把删除的操作放到 wait_pid 那里进行，同样会导致这个问题，因为没有保证在这个核上运行的任务已经回到了调度函数
// 2. 在这里更换页表，并且关闭中断，若产生中断，则下一次恢复执行时，会继续使用原来的页表
pub async fn exit(exit_code: isize) {
    let curr = current_task();
    let curr_id = curr.id().as_u64();

    let current_process = current_process().await;
    info!("exit task id {} with code _{}_", curr_id, exit_code);

    let exit_signal = current_process
        .signal_modules
        .lock()
        .await
        .get(&curr_id)
        .unwrap()
        .get_exit_signal();

    if exit_signal.is_some() || curr.is_leader() {
        let parent = current_process.get_parent();
        if parent != KERNEL_PROCESS_ID {
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
        if current_process
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
            for task in current_process.tasks.lock().await.deref() {
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
        current_process.exit_main_task().await;
        current_process.tasks.lock().await.clear();

        current_process.fd_manager.fd_table.lock().await.clear();

        current_process.signal_modules.lock().await.clear();

        let mut pid2pc = PID2PC.lock().await;
        let kernel_process = pid2pc.get(&KERNEL_PROCESS_ID).unwrap();
        // 将子进程交给idle进程
        // process.memory_set = Arc::clone(&kernel_process.memory_set);
        for child in current_process.children.lock().await.deref() {
            child.set_parent(KERNEL_PROCESS_ID);
            kernel_process.children.lock().await.push(Arc::clone(child));
        }
        if let Some(parent_process) = pid2pc.get(&current_process.get_parent()) {
            parent_process.set_vfork_block(false).await;
        }
        let kernel_pt_token = kernel_process.memory_set.lock().await.page_table_token();

        pid2pc.remove(&current_process.pid());
        drop(pid2pc);
        // 在这里直接更换为内核页表，并且关闭中断
        axhal::arch::disable_irqs();
        unsafe {
            axhal::arch::write_page_table_root0(kernel_pt_token.into());
        };
        current_process.set_exit_code(exit_code);
        current_process.set_zombie(true);
        drop(current_process);
    } else {
        TID2TASK.lock().await.remove(&curr_id);
        // 从进程中删除当前线程
        let mut tasks = current_process.tasks.lock().await;
        let len = tasks.len();
        for index in 0..len {
            if tasks[index].id().as_u64() == curr_id {
                tasks.remove(index);
                break;
            }
        }
        // 从进程中删除当前线程
        current_process.signal_modules.lock().await.remove(&curr_id);
    }
    curr.set_exit_code(exit_code);
    curr.set_state(TaskState::Exited);
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
    let curr = current_task();
    let scheduler = curr.get_scheduler();
    let mut scheduler_guard = scheduler.lock();
    scheduler_guard.set_priority(current_task().as_task_ref(), prio)
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
    let curr_process = current_process().await;
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
                info!("wait pid _{}_ with code _{}_", child.pid(), exit_code);
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
                info!("wait pid _{}_ with code _{:?}_", child.pid(), exit_code);
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
