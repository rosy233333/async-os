//! 提供与 net work 相关的 syscall

use crate::SyscallResult;
mod imp;

#[allow(unused)]
mod socket;
use imp::*;
pub use socket::Socket;
mod net_syscall_id;
pub use net_syscall_id::NetSyscallId::{self, *};

/// 进行 syscall 的分发
pub async fn net_syscall(
    syscall_id: net_syscall_id::NetSyscallId,
    args: [usize; 6],
) -> SyscallResult {
    match syscall_id {
        SOCKET => syscall_socket(args).await,
        BIND => syscall_bind(args).await,
        LISTEN => syscall_listen(args).await,
        ACCEPT => syscall_accept4(args).await,
        CONNECT => syscall_connect(args).await,
        GETSOCKNAME => syscall_get_sock_name(args).await,
        GETPEERNAME => syscall_getpeername(args).await,
        // GETPEERNAME => 0,
        SENDTO => syscall_sendto(args).await,
        RECVFROM => syscall_recvfrom(args).await,
        SETSOCKOPT => syscall_set_sock_opt(args).await,
        // SETSOCKOPT => 0,
        GETSOCKOPT => syscall_get_sock_opt(args).await,
        SOCKETPAIR => syscall_socketpair(args).await,
        ACCEPT4 => syscall_accept4(args).await,
        SHUTDOWN => syscall_shutdown(args).await,
        #[allow(unused)]
        _ => {
            panic!("Invalid Syscall Id: {:?}!", syscall_id);
            // return -1;
            // exit(-1)
        }
    }
}
