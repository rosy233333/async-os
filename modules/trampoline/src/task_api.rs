use alloc::{boxed::Box, format, sync::Arc};
use axhal::time::{current_time, TimeValue};
use core::{future::poll_fn, mem::ManuallyDrop, task::Poll, time::Duration};
pub use executor::*;
use kernel_guard::{BaseGuard, NoPreemptIrqSave};
use riscv::register::scause::{Exception, Trap};
use spin::Mutex;
use syscall::trap::{handle_page_fault, MappingFlags};
#[cfg(feature = "sched_taic")]
use syscall::LQS;

use crate::arch::check_trapframe;

// #[cfg(feature = "preempt")]
/// Checks if the current task should be preempted.
/// This api called after handle irq,it may be on a
/// disable_preempt ctx
pub fn current_check_preempt_pending(tf: &mut TrapFrame) {
    // 不需要特意实现抢占，因为从trap处理任务返回后，本来就是运行最高优先级的任务。
    // if let Some(curr) = current_task_may_uninit() {
    //     // if task is already exited or blocking,
    //     // no need preempt, they are rescheduling
    //     if curr.get_preempt_pending()
    //         && curr.can_preempt()
    //         && !curr.is_exited()
    //         && !curr.is_blocking()
    //     {
    //         trace!(
    //             "current {} is to be preempted in kernel, allow {}",
    //             curr.id_name(),
    //             curr.can_preempt()
    //         );
    //         curr.set_preempt_pending(false);
    //         tf.trap_status = TrapStatus::Blocked;
    //         set_task_tf(tf, CtxType::Interrupt);
    //     }
    // }
}

// #[cfg(feature = "preempt")]
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
    // #[cfg(feature = "thread-api")]
    // let res = thread_join(task);
    // #[cfg(not(feature = "thread-api"))]
    // let res = None;
    // JoinFuture::new(task.clone(), res).await
    TaskApiImpl::join(task).await
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
                    crate::handle_user_irq(tf.get_scause_code()).await;
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
                                tf.regs.a0, tf.regs.a1, tf.regs.a2, tf.regs.a3, tf.regs.a4,
                                tf.regs.a5,
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
                        let ktask_callback: Arc<spin::mutex::Mutex<Option<usize>>> =
                            Arc::new(Mutex::new(None));
                        let ktask_callback_clone = ktask_callback.clone();
                        let _pid = current_executor().await.pid() as usize;
                        let fut = Box::pin(async move {
                            let res = syscall::trap::handle_syscall(syscall_id, args).await;
                            // 将结果写回到用户态 SyscallFuture 的 res 中
                            unsafe {
                                let ret = ret_ptr as *mut Option<Result<usize, syscalls::Errno>>;
                                (*ret).replace(syscalls::Errno::from_ret(res as _));
                            }
                            #[cfg(feature = "sched_taic")]
                            // 唤醒 waker，获取 waker
                            if let Some(utask_ptr) = *ktask_callback_clone.lock() {
                                debug!("using taic wakeup mechanism {:#X}", utask_ptr);
                                // taic 控制器唤醒用户态任务
                                let lqs = LQS.lock().await;
                                let lq = lqs.get(&(1, _pid)).unwrap();
                                lq.task_enqueue(utask_ptr);
                            }
                            drop(ktask_callback_clone);
                            res
                        });
                        let ktask = current_executor()
                            .await
                            .new_ktask(format!("syscall {}", tf.regs.a7), fut)
                            .await;
                        debug!("new ktask about syscall {}", ktask.id_name());
                        unsafe {
                            CurrentTask::clean_current();
                            CurrentTask::init_current(ktask.clone());
                        }
                        let waker = current_task().waker();
                        let mut cx = core::task::Context::from_waker(&waker);
                        /************************************************************/
                        let res = if let Poll::Ready(res) = ktask.get_fut().as_mut().poll(&mut cx) {
                            CurrentTask::clean_current();
                            res
                        } else {
                            ktask.set_state(TaskState::Runable);
                            // ktask.get_scheduler().lock().add_task(ktask.clone());
                            CurrentTask::clean_current_without_drop();
                            let utask_ptr = tf.regs.t2;
                            ktask_callback.lock().replace(utask_ptr);
                            drop(ktask_callback);
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
                    // warn!("tf {:#X?}", tf);
                    handle_page_fault(stval.into(), MappingFlags::USER | MappingFlags::EXECUTE)
                        .await;
                }

                Trap::Exception(Exception::LoadPageFault) => {
                    // warn!("tf {:#X?}", tf);
                    handle_page_fault(stval.into(), MappingFlags::USER | MappingFlags::READ).await;
                }

                Trap::Exception(Exception::StorePageFault) => {
                    // warn!("tf {:#X?}", tf);
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
        debug!("before yield_now");
        // 关中断需要在设置任务状态前进行
        let state = NoPreemptIrqSave::acquire();
        #[cfg(feature = "thread-api")]
        {
            thread_yield();
            NoPreemptIrqSave::release(state);
            YieldFuture::new(Default::default())
        }
        #[cfg(not(feature = "thread-api"))]
        {
            current_task().set_state(TaskState::Runable);
            YieldFuture::new(state)
        }
    }

    fn block_current() -> BlockFuture {
        debug!("before block_current");
        #[cfg(feature = "thread-api")]
        thread_blocked();
        BlockFuture::new()
    }

    fn exit_current() -> ExitFuture {
        debug!("before exit_current");
        // 关中断需要在设置任务状态前进行
        let _state = NoPreemptIrqSave::acquire();
        #[cfg(feature = "thread-api")]
        {
            thread_exit();
            ExitFuture::new()
        }
        #[cfg(not(feature = "thread-api"))]
        {
            current_task().set_state(TaskState::Exited);
            ExitFuture::new()
        }
    }

    fn sleep(dur: Duration) -> SleepFuture {
        debug!("before sleep");
        // 关中断需要在设置任务状态前进行
        let state = NoPreemptIrqSave::acquire();
        #[cfg(feature = "thread-api")]
        {
            thread_sleep(dur + current_time());
            NoPreemptIrqSave::release(state);
            SleepFuture::new(current_time() + dur, Default::default())
        }
        #[cfg(not(feature = "thread-api"))]
        {
            SleepFuture::new(current_time() + dur, state)
        }
    }

    fn sleep_until(deadline: TimeValue) -> SleepFuture {
        debug!("before sleep_until");
        // 关中断需要在设置任务状态前进行
        let state = NoPreemptIrqSave::acquire();
        #[cfg(feature = "thread-api")]
        {
            thread_sleep(deadline);
            NoPreemptIrqSave::release(state);
            SleepFuture::new(deadline, Default::default())
        }
        #[cfg(not(feature = "thread-api"))]
        {
            SleepFuture::new(deadline, state)
        }
    }

    fn join(task: &TaskRef) -> JoinFuture {
        debug!("before join");
        // 关中断需要在设置任务状态前进行
        let state = NoPreemptIrqSave::acquire();
        #[cfg(feature = "thread-api")]
        {
            let res = thread_join(task);
            NoPreemptIrqSave::release(state);
            JoinFuture::new(task.clone(), res, Default::default())
        }
        #[cfg(not(feature = "thread-api"))]
        {
            JoinFuture::new(task.clone(), None, state)
        }
    }
}

// #[cfg(feature = "thread-api")]
pub fn thread_yield() {
    // let _guard = kernel_guard::NoPreemptIrqSave::acquire();
    // TrapFrame::thread_ctx(set_task_tf as usize, CtxType::Thread);
    current_task().set_state(TaskState::Runable);
    crate::vsched2::task::resched();
}

/// 注意：该函数不会设置任务状态，也不会关中断。
///
/// 应该先关中断、再将任务状态设置为Blocking、再调用该函数。
///
/// 这样设计的目的是保证任务在加入等待队列前就已经设置为Blocking。
// #[cfg(feature = "thread-api")]
pub fn thread_blocked() {
    // let _guard = kernel_guard::NoPreemptIrqSave::acquire();
    // TrapFrame::thread_ctx(set_task_tf as usize, CtxType::Thread);
    crate::vsched2::task::resched();
}

// #[cfg(feature = "thread-api")]
pub fn thread_sleep(deadline: TimeValue) {
    let waker = current_task().waker();
    task_api::set_alarm_wakeup(deadline, waker.clone());
    thread_blocked();
    task_api::cancel_alarm(&waker);
}

// #[cfg(feature = "thread-api")]
pub fn thread_exit() {
    // let _guard = kernel_guard::NoPreemptIrqSave::acquire();
    // TrapFrame::thread_ctx(set_task_tf as usize, CtxType::Thread);
    // let state = current_task().state();
    // warn!("before set_state, state: {:?}", state);
    current_task().set_state(TaskState::Exited);
    // let state = current_task().state();
    // warn!("after set_state, state: {:?}", state);
    crate::vsched2::task::resched();
}

// #[cfg(feature = "thread-api")]
pub fn thread_join(task: &TaskRef) -> Option<i32> {
    loop {
        let task_ptr = Arc::into_raw(task.clone());
        // warn!(
        //     "joined task {:#x} state: {:?}",
        //     task_ptr as usize,
        //     task.state()
        // );
        let _to_drop = unsafe { Arc::from_raw(task_ptr) };
        // 在将waker放入任务的waker队列期间，保持任务状态不变。
        let guard = task.state_lock_manual();
        if **guard == TaskState::Exited {
            drop(ManuallyDrop::into_inner(guard));
            return Some(task.get_exit_code() as i32);
        }
        warn!("{} join {}", current_task().id_name(), task.id_name());
        task.join(current_task().waker());
        drop(ManuallyDrop::into_inner(guard));
        // current_task().set_state(TaskState::Blocking);
        thread_blocked();
    }
}

// #[cfg(any(feature = "thread-api", feature = "preempt"))]
// pub fn set_task_tf(tf: &mut TrapFrame, ctx_type: CtxType) {
//     let curr = current_task();
//     let mut state = curr.state_lock_manual();
//     let new_kstack_top = curr.set_stack_ctx(tf as *const _, ctx_type);
//     // let new_kstack_top = taskctx::current_stack_top();
//     match **state {
//         // await 主动让权，将任务的状态修改为就绪后，放入就绪队列中
//         TaskState::Running => {
//             **state = TaskState::Runable;
//             curr.get_scheduler()
//                 .lock()
//                 .put_prev_task(curr.clone(), false);
//             CurrentTask::clean_current();
//         }
//         // 处于 Runable 状态的任务一定处于就绪队列中，不可能在 CPU 上运行
//         TaskState::Runable => panic!("Runable {} cannot be peding", curr.id_name()),
//         // 等待 Mutex 等进入到 Blocking 状态，但还在这个 CPU 上运行，
//         // 此时还没有被唤醒，因此将状态修改为 Blocked，等待被唤醒
//         TaskState::Blocking => {
//             **state = TaskState::Blocked;
//             CurrentTask::clean_current_without_drop();
//         }
//         // 由于等待 Mutex 等，导致进入到了 Blocking 状态，但在这里还没有修改状态为 Blocked 时
//         // 已经被其他 CPU 上运行的任务唤醒了，因此这里直接返回，让当前的任务继续执行
//         TaskState::Waked => {
//             **state = TaskState::Running;
//             drop(core::mem::ManuallyDrop::into_inner(state));
//             return;
//         }
//         // Blocked 状态的任务不可能在 CPU 上运行
//         TaskState::Blocked => panic!("Blocked {} cannot be pending", curr.id_name()),
//         // 退出的任务只能对应到 Poll::Ready
//         TaskState::Exited => panic!("Exited {} cannot be pending", curr.id_name()),
//     }
//     // 在这里释放锁，中间的过程不会发生中断
//     drop(core::mem::ManuallyDrop::into_inner(state));
//     unsafe {
//         core::arch::asm!(
//             "li a1, 0",
//             "li a2, 0",
//             "mv sp, {new_kstack_top}",
//             "j  {trampoline}",
//             new_kstack_top = in(reg) new_kstack_top,
//             trampoline = sym crate::trampoline,
//         )
//     }
// }

// #[cfg(any(feature = "thread-api", feature = "preempt"))]
/// 恢复上下文
///
/// 打开中断的时机：
///
/// - 线程返回时，在恢复的线程上下文中开中断。
/// - 中断返回时，在恢复上下文的同时打开了中断。
pub fn restore_from_stack_ctx(task: &TaskRef) {
    if let Some(StackCtx {
        kstack,
        trap_frame,
        ctx_type,
    }) = task.get_stack_ctx()
    {
        // log::info!("restore_from_stack_ctx: {:#x}", trap_frame as usize);
        // log::info!("{:#x?}", unsafe { &*trap_frame });

        // taskctx::put_prev_stack(kstack);

        // let sip = riscv::register::sip::read();
        // let type_str = match ctx_type {
        //     CtxType::Thread => "Thread",
        //     CtxType::Interrupt => "Interrupt",
        // };
        // log::info!(
        //     "restore_from_stack_ctx: \nctx_type: {},\nsip: timer: {}, software: {}, external: {}",
        //     type_str,
        //     sip.stimer(),
        //     sip.ssoft(),
        //     sip.sext()
        // );
        match ctx_type {
            CtxType::Thread => {
                // let base = kstack.as_ref().unwrap().top().as_usize();
                let tf = unsafe { &*trap_frame };
                if check_trapframe(tf, false) == false {
                    panic!("thread return trapframe invalid: trapframe {:#x?}", tf);
                }
                tf.thread_return()
            }
            // #[cfg(feature = "preempt")]
            CtxType::Interrupt => {
                // let base = kstack.as_ref().unwrap().top().as_usize();
                let tf = unsafe { &*trap_frame };
                if check_trapframe(tf, true) == false {
                    panic!("interrupt return trapframe invalid: trapframe {:#x?}", tf);
                }
                tf.preempt_return()
            }
        }
        panic!(
            "task ctx restore failed, kstack: {:?}, trap_frame: {:?}, ctx_type: {:?}",
            kstack, trap_frame, ctx_type
        );
    } else {
        panic!("cannot get stack ctx!");
    }
}
