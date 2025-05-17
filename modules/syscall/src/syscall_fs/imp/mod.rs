//! Implementations of the syscall about file system
extern crate alloc;

mod ctl;
mod epoll;
mod eventfd;
mod io;
mod link;
mod mount;
mod poll;
mod stat;
use axerrno::AxError;
pub use ctl::*;
pub use epoll::*;
pub use eventfd::*;
pub use io::*;
pub use link::*;
pub use mount::*;
pub use poll::*;
use process::link::{deal_with_path, FilePath};
pub use stat::*;

use crate::SyscallError;

/// To get the real path of the directory or the file by the given path and the directory fd.
pub async fn solve_path(
    dir_fd: usize,
    path_addr: Option<*const u8>,
    force_dir: bool,
) -> Result<FilePath, SyscallError> {
    match deal_with_path(dir_fd, path_addr, force_dir).await {
        Ok(path) => Ok(path),
        // Only invalid for file descriptor
        Err(AxError::InvalidInput) => Err(SyscallError::EBADF),
        Err(AxError::NotFound) => Err(SyscallError::ENOENT),
        Err(AxError::NotADirectory) => Err(SyscallError::ENOTDIR),
        Err(AxError::BadAddress) => Err(SyscallError::EFAULT),
        Err(_) => Err(SyscallError::EPERM),
    }
}
