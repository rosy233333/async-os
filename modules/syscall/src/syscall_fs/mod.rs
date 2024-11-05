//! 文件系统相关系统调用

pub mod ctype;
pub mod imp;

use crate::SyscallResult;
use async_fs::api::{File, OpenFlags};
use axerrno::AxResult;
pub use ctype::FileDesc;
mod fs_syscall_id;
pub use fs_syscall_id::FsSyscallId::{self, *};
extern crate alloc;
use imp::*;

/// 若使用多次new file打开同名文件，那么不同new file之间读写指针不共享，但是修改的内容是共享的
pub async fn new_file(path: &str, flags: &OpenFlags) -> AxResult<File> {
    let mut file = File::options();
    file.read(flags.readable());
    file.write(flags.writable());
    file.create(flags.creatable());
    file.create_new(flags.new_creatable());
    file.open(path).await
}

/// 文件系统相关系统调用
pub async fn fs_syscall(syscall_id: fs_syscall_id::FsSyscallId, args: [usize; 6]) -> SyscallResult {
    match syscall_id {
        OPENAT => syscall_openat(args).await,
        CLOSE => syscall_close(args).await,
        READ => syscall_read(args).await,
        WRITE => syscall_write(args).await,
        GETCWD => syscall_getcwd(args).await,
        PIPE2 => syscall_pipe2(args).await,
        DUP => syscall_dup(args).await,
        DUP3 => syscall_dup3(args).await,
        MKDIRAT => syscall_mkdirat(args).await,
        CHDIR => syscall_chdir(args).await,
        GETDENTS64 => syscall_getdents64(args).await,
        MOUNT => syscall_mount(args).await,
        UNMOUNT => syscall_umount(args).await,
        FSTAT => syscall_fstat(args).await,
        RENAMEAT | RENAMEAT2 => syscall_renameat2(args).await,
        READV => syscall_readv(args).await,
        WRITEV => syscall_writev(args).await,
        FCNTL64 => syscall_fcntl64(args).await,
        FSTATAT => syscall_fstatat(args).await,
        STATFS => syscall_statfs(args).await,
        FCHMODAT => syscall_fchmodat(args).await,
        FACCESSAT => syscall_faccessat(args).await,
        LSEEK => syscall_lseek(args).await,
        PREAD64 => syscall_pread64(args).await,
        PREADLINKAT => syscall_readlinkat(args).await,
        PWRITE64 => syscall_pwrite64(args).await,
        SENDFILE64 => syscall_sendfile64(args).await,
        FSYNC => Ok(0),
        FTRUNCATE64 => {
            syscall_ftruncate64(args).await
            // 0
        }
        IOCTL => syscall_ioctl(args).await,
        // 不做处理即可
        SYNC => Ok(0),
        COPYFILERANGE => syscall_copyfilerange(args).await,
        LINKAT => sys_linkat(args).await,
        UNLINKAT => syscall_unlinkat(args).await,
        SYMLINKAT => Ok(0),
        UTIMENSAT => syscall_utimensat(args).await,
        EPOLL_CREATE => syscall_epoll_create1(args).await,
        EPOLL_CTL => syscall_epoll_ctl(args).await,
        EPOLL_PWAIT => syscall_epoll_pwait(args).await,
        PPOLL => syscall_ppoll(args).await,
        PSELECT6 => syscall_pselect6(args).await,
        STATX => syscall_statx(args).await,
        PIDFD_OPEN => syscall_pidfd_open(args).await,
        FCHOWN => Ok(0),
        #[cfg(not(target_arch = "x86_64"))]
        EVENTFD => syscall_eventfd(args).await,
        // #[cfg(target_arch = "x86_64")]
        // // eventfd syscall in x86_64 does not support flags, use 0 instead
        // EVENTFD => syscall_eventfd([args[0], 0, 0, 0, 0, 0]),
        // #[cfg(target_arch = "x86_64")]
        // EVENTFD2 => syscall_eventfd(args),
        // #[cfg(target_arch = "x86_64")]
        // DUP2 => syscall_dup2(args),
        // #[cfg(target_arch = "x86_64")]
        // LSTAT => syscall_lstat(args),
        // #[cfg(target_arch = "x86_64")]
        // OPEN => syscall_open(args),
        // #[cfg(target_arch = "x86_64")]
        // PIPE => syscall_pipe(args),
        // #[cfg(target_arch = "x86_64")]
        // POLL => syscall_poll(args),
        // #[cfg(target_arch = "x86_64")]
        // STAT => syscall_stat(args),
        // #[cfg(target_arch = "x86_64")]
        // UNLINK => syscall_unlink(args),
        // #[cfg(target_arch = "x86_64")]
        // ACCESS => syscall_access(args),
        // #[cfg(target_arch = "x86_64")]
        // MKDIR => syscall_mkdir(args),
        // #[cfg(target_arch = "x86_64")]
        // RENAME => syscall_rename(args),
        // #[cfg(target_arch = "x86_64")]
        // RMDIR => syscall_rmdir(args),
        // #[cfg(target_arch = "x86_64")]
        // SELECT => syscall_select(args),
        // #[cfg(target_arch = "x86_64")]
        // READLINK => syscall_readlink(args),
        // #[cfg(target_arch = "x86_64")]
        // CREAT => syscall_creat(args),
        // #[cfg(target_arch = "x86_64")]
        // EPOLL_CREATE1 => syscall_epoll_create1(args),
        // // EPOLL_CREATE1 => unimplemented!("epoll_create1"),
        // #[cfg(target_arch = "x86_64")]
        // EPOLL_WAIT => syscall_epoll_wait(args),
        // // EPOLL_PWAIT => unimplemented!("epoll_ctl"),
        // #[cfg(target_arch = "x86_64")]
        // CHMOD => Ok(0),
        // #[cfg(target_arch = "x86_64")]
        // CHOWN => Ok(0),
        // #[cfg(target_arch = "x86_64")]
        // MKNOD => Ok(0),
        #[allow(unused)]
        _ => unimplemented!("syscall_id: {:?}", syscall_id),
    }
}
