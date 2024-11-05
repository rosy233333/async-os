//! 以协程的方式实现同步原语、以及任务调度模块中的 WaitQueue、TimerQueue
//! 目前支持的原语：
//! - [`Mutex`]: A mutual exclusion primitive.

#![cfg_attr(not(test), no_std)]
#![feature(ptr_metadata)]

extern crate alloc;

mod mutex;
pub use mutex::*;

mod wait_queue;
pub use wait_queue::*;
