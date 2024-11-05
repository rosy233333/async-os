//! 与内存相关的系统调用

use crate::SyscallResult;

mod imp;

mod mem_syscall_id;
pub use mem_syscall_id::MemSyscallId::{self, *};

use imp::*;
/// 与内存相关的系统调用
pub async fn mem_syscall(
    syscall_id: mem_syscall_id::MemSyscallId,
    args: [usize; 6],
) -> SyscallResult {
    match syscall_id {
        BRK => syscall_brk(args).await,
        MUNMAP => syscall_munmap(args).await,
        MREMAP => syscall_mremap(args).await,

        MMAP => syscall_mmap(args).await,
        MSYNC => syscall_msync(args).await,
        MPROTECT => syscall_mprotect(args).await,
        MEMBARRIER => Ok(0),
        SHMGET => syscall_shmget(args).await,
        SHMCTL => Ok(0),
        SHMAT => syscall_shmat(args).await,
        #[cfg(target_arch = "x86_64")]
        MLOCK => syscall_mlock(args),
        #[allow(unused)]
        _ => {
            panic!("Invalid Syscall Id: {:?}!", syscall_id);
        }
    }
}
