use axhal::time::{current_time, TimeValue};
use core::{future::poll_fn, task::Poll, time::Duration};
pub use executor::*;
use riscv::register::scause::{Exception, Trap};
use syscall::trap::{handle_page_fault, MappingFlags};
use alloc::{boxed::Box, format};

#[cfg(feature = "thread")]
use kernel_guard::BaseGuard;

#[cfg(feature = "preempt")]
/// Checks if the current task should be preempted.
/// This api called after handle irq,it may be on a
/// disable_preempt ctx
pub fn current_check_preempt_pending(tf: &mut TrapFrame) {
    if let Some(curr) = current_task_may_uninit() {
        // if task is already exited or blocking,
        // no need preempt, they are rescheduling
        if curr.get_preempt_pending()
            && curr.can_preempt()
            && !curr.is_exited()
            && !curr.is_blocking()
        {
            trace!(
                "current {} is to be preempted in kernel, allow {}",
                curr.id_name(),
                curr.can_preempt()
            );
            curr.set_preempt_pending(false);
            set_task_tf(tf, CtxType::Interrupt);
        }
    }
}

#[cfg(feature = "preempt")]
/// Checks if the current task should be preempted.
/// This api called after handle irq,it may be on a
/// disable_preempt ctx
pub async fn current_check_user_preempt_pending(_tf: &mut TrapFrame) {
    if let Some(curr) = current_task_may_uninit() {
        // if task is already exited or blocking,
        // no need preempt, they are rescheduling
        if curr.get_preempt_pending()
            && curr.can_preempt()
            && !curr.is_exited()
            && !curr.is_blocking()
        {
            trace!(
                "current {} is to be preempted in user mode, allow {}",
                curr.id_name(),
                curr.can_preempt()
            );
            curr.set_preempt_pending(false);
            _tf.trap_status = TrapStatus::Blocked;
            yield_now().await;
        }
    }
}


/// 这个接口还没有统一，后续还需要统一成两种接口都可以使用的形式
pub async fn wait(task: &TaskRef) -> Option<i32> {
    JoinFuture::new(task.clone(), None).await
}

pub async fn user_task_top() -> isize {
    loop {
        let curr = current_task();
        let mut tf = curr.utrap_frame().unwrap();
        if tf.trap_status == TrapStatus::Blocked {
            let trap = tf.get_scause_type();
            let stval = tf.stval;
            match trap {
                Trap::Interrupt(_interrupt) => {
                    crate::handle_user_irq(tf.get_scause_code(), &mut tf).await;
                }
                Trap::Exception(Exception::UserEnvCall) => {
                    axhal::arch::enable_irqs();
                    tf.sepc += 4;
                    // 简单的方式是根据参数的值进行不同的处理，根据参数进行不同的处理
                    let result = if tf.regs.t0 != crate::IS_ASYNC {
                        // 若没有传递指定的参数，则会按照阻塞的方式进行
                        syscall::trap::handle_syscall(
                            tf.regs.a7,
                            [
                                tf.regs.a0, tf.regs.a1, tf.regs.a2, tf.regs.a3, tf.regs.a4, tf.regs.a5,
                            ],
                        )
                        .await
                    } else {
                        /*  按照非阻塞的方式处理系统调用，新建一个属于当前进程的内核协程来执行，
                            在执行之前需要临时修改 CurrentTask 为新建的内核协程，
                            相当于这个内核协程临时抢占了原本的系统调用处理协程，
                            过程中如果产生了中断不会对原本的逻辑产生影响，

                            需要注意的是，在临时修改了 CurrentTask 之间（代码中使用/***/包括的部分）不允许使用 await 关键字，
                            因为 await 携带的信息是 curr 的信息，而不是新建的内核协程的信息，需要使用临时构建的 cx 来执行 poll 函数，
                            
                            1. 当这个内核协程返回 Pending 时，会将 EAGAIN 当作返回值传给用户态，用户态继续执行其他的协程
                            2. 当这个内核协程返回 Ready 时，会将内核协程的返回值传给用户态，用户态继续当前的协程
                        
                        */
                        let syscall_id = tf.regs.a7;
                        let args = [
                            tf.regs.a0, tf.regs.a1, tf.regs.a2, tf.regs.a3, tf.regs.a4, tf.regs.a5,
                        ];
                        let ret_ptr = tf.regs.t1;
                        let fut = Box::pin(async move { 
                            let res = syscall::trap::handle_syscall(syscall_id, args).await;
                            // 将结果写回到用户态 SyscallFuture 的 res 中
                            unsafe {
                                let ret = ret_ptr as *mut Option<Result<usize, syscalls::Errno>>;
                                (*ret).replace(syscalls::Errno::from_ret(res as _));
                            }
                            res
                        });
                        let ktask = current_executor().await.new_ktask(
                            format!("syscall {}", tf.regs.a7), 
                            fut
                        ).await;
                        debug!("new ktask about syscall {}", ktask.id_name());
                        let waker = waker_from_task(&ktask);
                        let mut cx = core::task::Context::from_waker(&waker);
                        unsafe { 
                            CurrentTask::clean_current();
                            CurrentTask::init_current(ktask.clone()); 
                        }
                        /************************************************************/
                        let res = if let Poll::Ready(res) = ktask.get_fut().as_mut().poll(&mut cx) {
                            CurrentTask::clean_current();
                            res
                        } else {
                            CurrentTask::clean_current_without_drop();
                            axerrno::LinuxError::EAGAIN as isize
                        };
                        /************************************************************/
                        unsafe {
                            CurrentTask::init_current(curr.clone());
                        }
                        res
                    };
                    // 判断任务是否退出
                    if curr.is_exited() {
                        // 任务结束，需要切换至其他任务，关中断
                        axhal::arch::disable_irqs();
                        return curr.get_exit_code() as isize;
                    }
                    if -result == syscall::SyscallError::ERESTART as isize {
                        // Restart the syscall
                        tf.rewind_pc();
                    } else {
                        tf.regs.a0 = result as usize;
                    }
                    axhal::arch::disable_irqs();
                }
                Trap::Exception(Exception::InstructionPageFault) => {
                    handle_page_fault(stval.into(), MappingFlags::USER | MappingFlags::EXECUTE)
                        .await;
                }

                Trap::Exception(Exception::LoadPageFault) => {
                    handle_page_fault(stval.into(), MappingFlags::USER | MappingFlags::READ).await;
                }

                Trap::Exception(Exception::StorePageFault) => {
                    handle_page_fault(stval.into(), MappingFlags::USER | MappingFlags::WRITE).await;
                }

                _ => {
                    panic!(
                        "Unhandled trap {:?} @ {:#x}:\n{:#x?}",
                        tf.get_scause_type(),
                        tf.sepc,
                        tf
                    );
                }
            }
            syscall::trap::handle_signals().await;
            tf.trap_status = TrapStatus::Done;
            // 判断任务是否退出
            if curr.is_exited() {
                // 任务结束，需要切换至其他任务，关中断
                axhal::arch::disable_irqs();
                return curr.get_exit_code() as isize;
            }
        }
        poll_fn(|_cx| {
            if tf.trap_status == TrapStatus::Done {
                Poll::Pending
            } else {
                Poll::Ready(())
            }
        })
        .await
    }
}

struct TaskApiImpl;

#[crate_interface::impl_interface]
impl task_api::TaskApi for TaskApiImpl {
    fn current_task() -> CurrentTask {
        current_task()
    }

    fn yield_now() -> YieldFuture {
        #[cfg(feature = "thread")]
        thread_yield();
        YieldFuture::new()
    }

    fn block_current() -> BlockFuture {
        current_task().set_state(TaskState::Blocked);
        #[cfg(feature = "thread")]
        thread_blocked();
        BlockFuture::new()
    }

    fn exit_current() -> ExitFuture {
        current_task().set_state(TaskState::Exited);
        #[cfg(feature = "thread")]
        thread_exit();
        ExitFuture::new()
    }

    fn sleep(dur: Duration) -> SleepFuture {
        #[cfg(feature = "thread")]
        thread_sleep(dur + current_time());
        SleepFuture::new(current_time() + dur)
    }

    fn sleep_until(deadline: TimeValue) -> SleepFuture {
        #[cfg(feature = "thread")]
        thread_sleep(deadline);
        SleepFuture::new(deadline)
    }

    fn join(task: &TaskRef) -> JoinFuture {
        #[cfg(feature = "thread")]
        let res = thread_join(task);
        #[cfg(not(feature = "thread"))]
        let res = None;
        JoinFuture::new(task.clone(), res)
    }
}

#[cfg(feature = "thread")]
pub fn thread_yield() {
    let _guard = kernel_guard::NoPreemptIrqSave::acquire();
    TrapFrame::thread_ctx(set_task_tf as usize, CtxType::Thread);
}

#[cfg(feature = "thread")]
pub fn thread_blocked() {
    let _guard = kernel_guard::NoPreemptIrqSave::acquire();
    TrapFrame::thread_ctx(set_task_tf as usize, CtxType::Thread);
}

#[cfg(feature = "thread")]
pub fn thread_sleep(deadline: TimeValue) {
    let waker = current_task().waker();
    task_api::set_alarm_wakeup(deadline, waker.clone());
    thread_blocked();
    task_api::cancel_alarm(&waker);
}

#[cfg(feature = "thread")]
pub fn thread_exit() {
    let _guard = kernel_guard::NoPreemptIrqSave::acquire();
    TrapFrame::thread_ctx(set_task_tf as usize, CtxType::Thread);
}

#[cfg(feature = "thread")]
pub fn thread_join(_task: &TaskRef) -> Option<i32> {
    loop {
        if _task.state() == TaskState::Exited {
            return Some(_task.get_exit_code() as i32);
        }
        _task.join(current_task().waker());
        current_task().set_state(TaskState::Blocked);
        thread_blocked();
    }
}

#[cfg(any(feature = "thread", feature = "preempt"))]
pub fn set_task_tf(tf: &mut TrapFrame, ctx_type: CtxType) {
    let curr = current_task();
    curr.set_stack_ctx(tf as *const _, ctx_type);
    // let raw_task_ptr = CurrentTask::clean_current_without_drop();
    let new_kstack_top = taskctx::current_stack_top();
    if curr.state() == TaskState::Running {
        warn!("set_task_tf wake up {}", curr.id_name());
        wakeup_task(curr.clone());
    }
    CurrentTask::clean_current();
    unsafe {
        core::arch::asm!(
            "li a1, 0",
            "li a2, 0",
            "mv sp, {new_kstack_top}",
            "j  {trampoline}",
            new_kstack_top = in(reg) new_kstack_top,
            trampoline = sym crate::trampoline,
        )
    }
}

#[cfg(any(feature = "thread", feature = "preempt"))]
pub fn restore_from_stack_ctx(task: &TaskRef) {
    if let Some(StackCtx {
        kstack,
        trap_frame,
        ctx_type,
    }) = task.get_stack_ctx()
    {
        taskctx::put_prev_stack(kstack);
        match ctx_type {
            CtxType::Thread => unsafe { &*trap_frame }.thread_return(),
            #[cfg(feature = "preempt")]
            CtxType::Interrupt => unsafe { &*trap_frame }.preempt_return(),
        }
    }
}
