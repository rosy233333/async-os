use alloc::sync::Arc;
use process::current_executor;

use crate::syscall_fs::ctype::eventfd::EventFd;
use crate::{SyscallError, SyscallResult};

pub async fn syscall_eventfd(args: [usize; 6]) -> SyscallResult {
    let initval = args[0] as u64;
    let flags = args[1] as u32;

    let process = current_executor().await;
    let mut fd_table = process.fd_manager.fd_table.lock().await;
    let fd_num = if let Ok(fd) = process.alloc_fd(&mut fd_table) {
        fd
    } else {
        return Err(SyscallError::EPERM);
    };

    fd_table[fd_num] = Some(Arc::new(EventFd::new(initval, flags)));

    Ok(fd_num as isize)
}
