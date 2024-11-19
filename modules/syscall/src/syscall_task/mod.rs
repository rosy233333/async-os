//! 提供和 task 模块相关的 syscall

mod task_syscall_id;

use crate::SyscallResult;
pub use task_syscall_id::TaskSyscallId::{self, *};

mod imp;

pub use imp::*;

/// 进行 syscall 的分发
pub async fn task_syscall(
    syscall_id: task_syscall_id::TaskSyscallId,
    args: [usize; 6],
) -> SyscallResult {
    match syscall_id {
        EXIT => syscall_exit(args).await,
        EXECVE => syscall_exec(args).await,
        CLONE => syscall_clone(args).await,
        CLONE3 => syscall_clone3(args).await,
        NANO_SLEEP => syscall_sleep(args).await,
        SCHED_YIELD => syscall_yield().await,
        TIMES => syscall_time(args),
        UNAME => syscall_uname(args),
        GETTIMEOFDAY => syscall_get_time_of_day(args),
        GETPGID => syscall_getpgid(),
        SETPGID => syscall_setpgid(args),
        GETPID => syscall_getpid().await,

        GETPPID => syscall_getppid().await,
        WAIT4 => syscall_wait4(args).await,
        GETRANDOM => syscall_getrandom(args).await,

        SIGSUSPEND => syscall_sigsuspend(args).await,

        SIGACTION => syscall_sigaction(args).await,

        KILL => syscall_kill(args).await,

        TKILL => syscall_tkill(args).await,

        TGKILL => syscall_tgkill(args).await,

        SIGPROCMASK => syscall_sigprocmask(args).await,
        SIGALTSTACK => syscall_sigaltstack(args).await,
        SIGRETURN => syscall_sigreturn().await,
        EXIT_GROUP => syscall_exit(args).await,
        SET_TID_ADDRESS => syscall_set_tid_address(args).await,
        PRLIMIT64 => syscall_prlimit64(args).await,
        CLOCK_GET_TIME => syscall_clock_get_time(args),
        GETUID => syscall_getuid(),
        GETEUID => syscall_geteuid(),
        GETGID => syscall_getgid(),
        SETGID => Ok(0),
        GETEGID => syscall_getegid(),
        GETTID => syscall_gettid(),
        FUTEX => syscall_futex(args).await,
        SET_ROBUST_LIST => syscall_set_robust_list(args).await,
        GET_ROBUST_LIST => syscall_get_robust_list(args).await,
        SYSINFO => syscall_sysinfo(args).await,
        SETITIMER => syscall_settimer(args).await,
        GETTIMER => syscall_gettimer(args).await,
        SETSID => syscall_setsid().await,
        GETRUSAGE => syscall_getrusage(args).await,
        UMASK => syscall_umask(args).await,
        // 不做处理即可
        SIGTIMEDWAIT => Ok(0),
        SYSLOG => Ok(0),
        MADVICE => Ok(0),
        SCHED_SETAFFINITY => Ok(0),
        SCHED_GETAFFINITY => syscall_sched_getaffinity(args).await,
        SCHED_SETSCHEDULER => syscall_sched_setscheduler(args).await,
        SCHED_GETSCHEDULER => syscall_sched_getscheduler(args).await,
        GET_MEMPOLICY => Ok(0),
        CLOCK_GETRES => syscall_clock_getres(args).await,
        CLOCK_NANOSLEEP => syscall_clock_nanosleep(args).await,
        PRCTL => syscall_prctl(args).await,
        PIDFD_SEND_SIGNAL => syscall_pidfd_send_signal(args).await,
        // syscall below just for x86_64
        #[cfg(target_arch = "x86_64")]
        VFORK => syscall_vfork(),
        #[cfg(target_arch = "x86_64")]
        ARCH_PRCTL => syscall_arch_prctl(args),
        #[cfg(target_arch = "x86_64")]
        FORK => syscall_fork(),
        #[cfg(target_arch = "x86_64")]
        ALARM => Ok(0),
        #[cfg(target_arch = "x86_64")]
        RSEQ => Ok(0),
        #[cfg(target_arch = "x86_64")]
        TIME => Ok(0),
        #[allow(unused)]
        _ => {
            panic!("Invalid Syscall Id: {:?}!", syscall_id);
            // return -1;
            // exit(-1)
        }
    }
}
