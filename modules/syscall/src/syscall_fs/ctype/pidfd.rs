extern crate alloc;
use crate::{SyscallError, SyscallResult};
use alloc::boxed::Box;
use alloc::sync::Arc;
use async_fs::api::{FileIO, OpenFlags};
use async_io::SeekFrom;
use axerrno::{AxError, AxResult};
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
        self.process.pid().as_u64()
    }
}
#[async_trait::async_trait]
impl FileIO for PidFd {
    async fn read(&self, _buf: &mut [u8]) -> AxResult<usize> {
        Err(axerrno::AxError::Unsupported)
    }

    async fn write(&self, _buf: &[u8]) -> AxResult<usize> {
        Err(AxError::Unsupported)
    }

    async fn seek(&self, _pos: SeekFrom) -> AxResult<u64> {
        Err(AxError::Unsupported)
    }

    /// To check whether the target process is still alive
    async fn readable(&self) -> bool {
        self.process.get_zombie()
    }

    async fn writable(&self) -> bool {
        false
    }

    async fn executable(&self) -> bool {
        false
    }

    async fn get_type(&self) -> async_fs::api::FileIOType {
        async_fs::api::FileIOType::Other
    }

    async fn get_status(&self) -> OpenFlags {
        self.flags.lock().await.clone()
    }

    async fn set_status(&self, flags: OpenFlags) -> bool {
        *self.flags.lock().await = flags;
        true
    }

    async fn set_close_on_exec(&self, is_set: bool) -> bool {
        if is_set {
            // 设置close_on_exec位置
            *self.flags.lock().await |= OpenFlags::CLOEXEC;
        } else {
            *self.flags.lock().await &= !OpenFlags::CLOEXEC;
        }
        true
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
