use core::{cell::Cell, future::{poll_fn, Future}, task::{Context, Poll, Waker}};
use alloc::boxed::Box;

/// 实现系统调用接口需要用到的用户态任务操作
#[crate_interface::def_interface]
pub trait TaskOps {
    /// 线程的让出函数
    #[cfg(feature = "thread")]
    fn yield_now();
    
    /// 修改协程的任务状态，使得协程在返回Pending时视为让出，直接放回就绪队列。
    #[cfg(not(feature = "thread"))]
    fn set_state_yield();
}