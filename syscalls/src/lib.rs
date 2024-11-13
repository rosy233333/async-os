//! 这个仓库对原来的 syscall 结果进行封装，提供同步和异步的系统调用接口
//!
//!
//!
//!

#![feature(anonymous_pipe)]

mod fut;
mod syscall;

pub use fut::SyscallFuture;
pub use syscall::*;

use syscalls::Sysno;
use syscalls::Errno;

pub mod raw {
    //! Exposes raw syscalls that simply return a `usize` instead of a `Result`.

    pub use syscalls::raw_syscall;
}

