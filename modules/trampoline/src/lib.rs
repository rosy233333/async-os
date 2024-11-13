#![no_std]
#![feature(naked_functions)]
#![feature(fn_align)]
#![feature(stmt_expr_attributes)]
#![feature(doc_cfg)]

extern crate alloc;
#[macro_use]
extern crate log;

mod arch;
mod executor_api;
mod fs_api;
mod init_api;
mod task_api;
mod trap_api;

use alloc::sync::Arc;
pub use arch::init_interrupt;
use core::task::{Context, Poll};
pub use fs_api::fs_init;
pub use init_api::*;
pub use taskctx::TrapFrame;

pub use executor_api::*;
use riscv::register::scause::{self, Trap};
pub use task_api::*;
pub use trap_api::*;

/// 进入 Trampoline 的方式：
///   1. 初始化后函数调用：没有 Trap，但存在就绪任务
///   2. 内核发生 Trap：存在任务被打断（CurrentTask 不为空），或者没有任务被打断（CurrentTask 为空）
///   3. 用户态发生 Trap：任务被打断，CurrentTask 不为空
///
/// 内核发生 Trap 时，将 TrapFrame 保存在内核栈上
/// 在用户态发生 Trap 时，将 TrapFrame 直接保存在任务控制块中，而不是在内核栈上
///
/// 只有通过 trap 进入这个入口时，是处于关中断的状态，剩下的任务切换是没有关中断
#[no_mangle]
pub fn trampoline(tf: &mut TrapFrame, has_trap: bool, from_user: bool) {
    loop {
        if !from_user && has_trap {
            // 在内核中发生了 Trap，只处理中断，目前还不支持抢占，因此是否有任务被打断是不做处理的
            // warn!("here");
            let scause = scause::read();
            match scause.cause() {
                Trap::Interrupt(_interrupt) => handle_irq(tf.get_scause_code(), tf),
                Trap::Exception(e) => {
                    panic!(
                        "Unsupported kernel trap {:?} @ {:#x}:\n{:#x?}",
                        e, tf.sepc, tf
                    )
                }
            }
            return;
        } else {
            // 用户态发生了 Trap 或者需要调度
            if let Some(task) = CurrentTask::try_get().or_else(|| {
                if let Some(task) = CurrentExecutor::get().pick_next_task() {
                    unsafe {
                        CurrentTask::init_current(task);
                    }
                    Some(CurrentTask::get())
                } else {
                    None
                }
            }) {
                run_task(task.as_task_ref());
            } else {
                axhal::arch::enable_irqs();
                // 如果当前的 Executor 中没有任务了，则切换回内核的 Executor
                turn_to_kernel_executor();
                // 没有就绪任务，等待中断
                #[cfg(feature = "irq")]
                axhal::arch::wait_for_irqs();
            }
        }
    }
}

const IS_ASYNC: usize = 0x5f5f5f5f;

pub fn run_task(task: &TaskRef) {
    let waker = taskctx::waker_from_task(task);
    let cx = &mut Context::from_waker(&waker);
    let page_table_token = task.get_page_table_token();
    if page_table_token != 0 {
        unsafe {
            axhal::arch::write_page_table_root0(page_table_token.into());
        };
    }
    #[cfg(any(feature = "thread", feature = "preempt"))]
    restore_from_stack_ctx(&task);
    // warn!("run task {} count {}", task.id_name(), Arc::strong_count(task));
    let res = task.get_fut().as_mut().poll(cx);
    match res {
        Poll::Ready(exit_code) => {
            debug!("task exit: {}, exit_code={}", task.id_name(), exit_code);
            task.set_state(TaskState::Exited);
            task.set_exit_code(exit_code);
            task.notify_waker_for_exit();
            if task.is_init() {
                assert!(
                    Arc::strong_count(&task) == 1,
                    "count {}",
                    Arc::strong_count(&task)
                );
                axhal::misc::terminate();
            }
            CurrentTask::clean_current();
        }
        Poll::Pending => {
            if let Some(tf) = task.utrap_frame() {
                if tf.trap_status == TrapStatus::Done {
                    tf.kernel_sp = taskctx::current_stack_top();
                    tf.scause = 0;
                    // 这里不能打开中断
                    axhal::arch::disable_irqs();
                    unsafe {
                        tf.user_return();
                    }
                } else {
                    if tf.get_syscall_args().iter().find(|&&x| x == IS_ASYNC).is_some() {
                        tf.trap_status = TrapStatus::Done;
                        tf.regs.a0 = axerrno::LinuxError::EAGAIN as usize;
                        tf.kernel_sp = taskctx::current_stack_top();
                        tf.scause = 0;
                        // 这里不能打开中断
                        axhal::arch::disable_irqs();
                        unsafe {
                            tf.user_return();
                        }
                    }
                }
            }
            CurrentTask::clean_current_without_drop();
        }
    }
}
