extern crate alloc;
use crate::{SyscallError, SyscallResult};
use alloc::sync::Arc;
use async_fs::api::{FileIO, OpenFlags};
use async_io::SeekFrom;
use axerrno::{AxError, AxResult};
use core::{
    future::Future,
    pin::Pin,
    task::{ready, Context, Poll},
};
use executor::{current_executor, Executor, PID2PC};
use sync::Mutex;

pub struct PidFd {
    flags: Mutex<OpenFlags>,
    process: Arc<Executor>,
}

impl PidFd {
    /// Create a new PidFd
    pub fn new(process: Arc<Executor>, flags: OpenFlags) -> Self {
        Self {
            flags: Mutex::new(flags),
            process,
        }
    }

    #[allow(unused)]
    pub fn pid(&self) -> u64 {
        self.process.pid()
    }
}
impl FileIO for PidFd {
    fn read(self: Pin<&Self>, _cx: &mut Context<'_>, _buf: &mut [u8]) -> Poll<AxResult<usize>> {
        Poll::Ready(Err(AxError::Unsupported))
    }

    fn write(self: Pin<&Self>, _cx: &mut Context<'_>, _buf: &[u8]) -> Poll<AxResult<usize>> {
        Poll::Ready(Err(AxError::Unsupported))
    }

    fn seek(self: Pin<&Self>, _cx: &mut Context<'_>, _pos: SeekFrom) -> Poll<AxResult<u64>> {
        Poll::Ready(Err(AxError::Unsupported))
    }

    /// To check whether the target process is still alive
    fn readable(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<bool> {
        Poll::Ready(self.process.get_zombie())
    }

    fn writable(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<bool> {
        Poll::Ready(false)
    }

    fn executable(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<bool> {
        Poll::Ready(false)
    }

    fn get_type(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<async_fs::api::FileIOType> {
        Poll::Ready(async_fs::api::FileIOType::Other)
    }

    fn get_status(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<OpenFlags> {
        Pin::new(&mut self.flags.lock())
            .poll(cx)
            .map(|flags| *flags)
    }

    fn set_status(self: Pin<&Self>, cx: &mut Context<'_>, flags: OpenFlags) -> Poll<bool> {
        *ready!(Pin::new(&mut self.flags.lock()).poll(cx)) = flags;
        Poll::Ready(true)
    }

    fn set_close_on_exec(self: Pin<&Self>, cx: &mut Context<'_>, is_set: bool) -> Poll<bool> {
        let mut flags = ready!(Pin::new(&mut self.flags.lock()).poll(cx));
        if is_set {
            // 设置close_on_exec位置
            *flags |= OpenFlags::CLOEXEC;
        } else {
            *flags &= !OpenFlags::CLOEXEC;
        }
        Poll::Ready(true)
    }
}

pub async fn new_pidfd(pid: u64, mut flags: OpenFlags) -> SyscallResult {
    // It is set to close the file descriptor on exec
    flags |= OpenFlags::CLOEXEC;
    let pid2fd = PID2PC.lock().await;

    let pidfd = pid2fd
        .get(&pid)
        .map(|target_process| PidFd::new(Arc::clone(target_process), flags))
        .ok_or(SyscallError::EINVAL)?;
    drop(pid2fd);
    let process = current_executor();
    let mut fd_table = process.fd_manager.fd_table.lock().await;
    let fd = process
        .alloc_fd(&mut fd_table)
        .map_err(|_| SyscallError::EMFILE)?;
    fd_table[fd] = Some(Arc::new(pidfd));
    Ok(fd as isize)
}
