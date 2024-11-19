/// 这里使用 read 来作为第一个异步系统调用的例子
/// 
/// 但在此之前，是否要提供注册和取消异步系统调用的同步系统调用接口呢？
/// 内核默认提供异步系统调用的环境，在必要的时候，才禁用异步系统调用
/// 异步不一定需要多核，单核也可以异步，单核情况下，通过 ecall 来通知内核
/// 

use cfg_if::cfg_if;
use crate::{SyscallFuture, Sysno};

pub fn sys_read(fd: i32, buf: &mut [u8]) -> SyscallFuture {
    fut_adapter(SyscallFuture::new(Sysno::read, &[fd as usize, buf.as_mut_ptr() as usize, buf.len()]))
}

pub fn sys_write(fd: i32, buf: &[u8]) -> SyscallFuture {
    fut_adapter(SyscallFuture::new(Sysno::write, &[fd as usize, buf.as_ptr() as usize, buf.len()]))
}

/// 用于预处理新建的`SyscallFuture`，使系统调用接口支持async/non-async、await/non-await、blocking/non-blocking的不同组合。
fn fut_adapter(mut sf: SyscallFuture) -> SyscallFuture {
    cfg_if! {
        if #[cfg(feature = "thread")] {
            // non-await式调用
            cfg_if! {
                if #[cfg(feature = "blocking")] {
                    // 阻塞式系统调用，可以一次返回结果
                    sf.has_issued = true;
                    sf.run();
                }
                else {
                    // 非阻塞式系统调用，因为non-await，因此（用户态）让出操作在该函数内完成。
                    sf.has_issued = true;
                    sf.run();
                    while !sf.is_finished() {
                        user_task_scheduler::yield_now();
                    }
                }
            }
            sf
        }
        else {
            // await式调用，不需处理SyscallFuture
            sf
        }
    }
}
