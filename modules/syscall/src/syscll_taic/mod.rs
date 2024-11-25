mod taic_syscall_id;

use crate::SyscallResult;
pub use taic_syscall_id::TaicSyscallId::{self, *};

mod imp;

pub use imp::*;

/// 进行 syscall 的分发
pub async fn taic_syscall(
    syscall_id: taic_syscall_id::TaicSyscallId,
    _args: [usize; 6],
) -> SyscallResult {
    match syscall_id {
        GET_TAIC => syscall_get_taic().await,
        #[allow(unused)]
        _ => {
            panic!("Invalid Syscall Id: {:?}!", syscall_id);
            // return -1;
            // exit(-1)
        }
    }
}
