//! 这个仓库对原来的 syscall 结果进行封装，提供同步和异步的系统调用接口
//!
//!
//!
//!
#![no_std]
#![allow(unused)]
#![feature(noop_waker)]

extern crate alloc;

mod fut;
mod raw_syscall;
mod syscall;
mod task_trait;

pub use fut::SyscallFuture;
pub use syscall::*;
pub use task_trait::TaskOps;

pub use syscalls::Errno;
use syscalls::Sysno;

pub mod raw {
    //! Exposes raw syscalls that simply return a `usize` instead of a `Result`.
    pub use super::raw_syscall::*;
}

#[repr(usize)]
pub(crate) enum AsyncFlags {
    ASYNC = 0x5f5f5f5f,
    SYNC = 0,
}
