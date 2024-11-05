//! 支持 futex 相关的 syscall

extern crate alloc;

use core::time::Duration;

use axlog::{debug, error};
use executor::{current_executor, current_task, futex::FutexRobustList};

use crate::{RobustList, SyscallError, SyscallResult, TimeSecs};

use axfutex::flags::*;
use executor::futex::{futex_requeue, futex_wait, futex_wake, futex_wake_bitset};

pub async fn syscall_futex(args: [usize; 6]) -> SyscallResult {
    let uaddr = args[0];
    let futex_op = args[1] as i32;
    let val = args[2] as u32;
    /* arg[3] is time_out_val or val2 depends on futex_op */
    let val2 = args[3];
    let uaddr2 = args[4];
    let mut val3 = args[5] as u32;

    let process = current_executor();
    // convert `TimeSecs` struct to `timeout` nanoseconds
    let timeout = if val2 != 0 && process.manual_alloc_for_lazy(val2.into()).await.is_ok() {
        let time_sepc: TimeSecs = unsafe { *(val2 as *const TimeSecs) };
        time_sepc.turn_to_nanos()
    } else {
        // usize::MAX
        0
    };

    let flags: i32 = futex_op_to_flag(futex_op);
    // cmd determines the operation of futex
    let cmd: i32 = futex_op & FUTEX_CMD_MASK;
    // TODO: shared futex and real time clock
    // It's Ok for ananonymous mmap to use private futex
    if (flags & FLAGS_SHARED) != 0 {
        debug!(
            "shared futex is not supported, but it's ok for anonymous mmap to use private futex"
        );
    }
    if (flags & FLAGS_CLOCKRT) != 0 {
        panic!("FUTEX_CLOCK_REALTIME is not supported");
    }
    match cmd {
        FUTEX_WAIT => {
            val3 = FUTEX_BITSET_MATCH_ANY;
            // convert relative timeout to absolute timeout
            let deadline: Option<Duration> = if timeout != 0 {
                Some(Duration::from_nanos(timeout as u64) + axhal::time::current_time())
            } else {
                None
            };
            futex_wait(uaddr.into(), flags, val, deadline, val3).await
        }
        FUTEX_WAIT_BITSET => {
            let deadline: Option<Duration> = if timeout != 0 {
                Some(Duration::from_nanos(timeout as u64))
            } else {
                None
            };
            futex_wait(uaddr.into(), flags, val, deadline, val3).await
        }
        FUTEX_WAKE => futex_wake(uaddr.into(), flags, val).await,
        FUTEX_WAKE_BITSET => futex_wake_bitset(uaddr.into(), flags, val, val3).await,
        FUTEX_REQUEUE => futex_requeue(uaddr.into(), flags, val, uaddr2.into(), val2 as u32).await,
        FUTEX_CMP_REQUEUE => {
            error!("[linux_syscall_api] futex: unsupported futex operation: FUTEX_CMP_REQUEUE");
            return Err(SyscallError::ENOSYS);
        }
        FUTEX_WAKE_OP => {
            // futex_wake(uaddr, flags, uaddr2, val, val2, val3)
            error!("[linux_syscall_api] futex: unsupported futex operation: FUTEX_WAKE_OP");
            return Err(SyscallError::ENOSYS);
        }
        // TODO: priority-inheritance futex
        _ => {
            error!(
                "[linux_syscall_api] futex: unsupported futex operation: {}",
                cmd
            );
            return Err(SyscallError::ENOSYS);
        }
    }
    // success anyway and reach here
}

/// 内核只发挥存储的作用
/// 但要保证head对应的地址已经被分配
/// # Arguments
/// * head: usize
/// * len: usize
pub async fn syscall_set_robust_list(args: [usize; 6]) -> SyscallResult {
    let head = args[0];
    let len = args[1];
    let process = current_executor();
    if len != core::mem::size_of::<RobustList>() {
        return Err(SyscallError::EINVAL);
    }
    let curr_id = current_task().id().as_u64();
    if process.manual_alloc_for_lazy(head.into()).await.is_ok() {
        let mut robust_list = process.robust_list.lock();
        robust_list.insert(curr_id, FutexRobustList::new(head, len));
        Ok(0)
    } else {
        Err(SyscallError::EINVAL)
    }
}

/// 取出对应线程的robust list
/// # Arguments
/// * pid: i32
/// * head: *mut usize
/// * len: *mut usize
pub async fn syscall_get_robust_list(args: [usize; 6]) -> SyscallResult {
    let pid = args[0] as i32;
    let head = args[1] as *mut usize;
    let len = args[2] as *mut usize;

    if pid == 0 {
        let process = current_executor();
        let curr_id = current_task().id().as_u64();
        if process
            .manual_alloc_for_lazy((head as usize).into())
            .await
            .is_ok()
        {
            let robust_list = process.robust_list.lock();
            if robust_list.contains_key(&curr_id) {
                let list = robust_list.get(&curr_id).unwrap();
                unsafe {
                    *head = list.head;
                    *len = list.len;
                }
            } else {
                return Err(SyscallError::EPERM);
            }
            return Ok(0);
        }
        return Err(SyscallError::EPERM);
    }
    Err(SyscallError::EPERM)
}
