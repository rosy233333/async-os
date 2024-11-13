/// 这里使用 read 来作为第一个异步系统调用的例子
/// 
/// 但在此之前，是否要提供注册和取消异步系统调用的同步系统调用接口呢？
/// 内核默认提供异步系统调用的环境，在必要的时候，才禁用异步系统调用
/// 异步不一定需要多核，单核也可以异步，单核情况下，通过 ecall 来通知内核
/// 

use crate::{SyscallFuture, Sysno};

pub fn sys_read(fd: i32, buf: &mut [u8]) -> SyscallFuture {
    SyscallFuture::new(Sysno::read, &[fd as usize, buf.as_mut_ptr() as usize, buf.len()])
}

pub fn sys_write(fd: i32, buf: &[u8]) -> SyscallFuture {
    SyscallFuture::new(Sysno::write, &[fd as usize, buf.as_ptr() as usize, buf.len()])
}
