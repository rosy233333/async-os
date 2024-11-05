#![cfg_attr(not(test), no_std)]
#![allow(async_fn_in_trait)]

#[cfg(test)]
mod tests;

/// 这个是提供给应用程序的接口
/// 以 async 的形式提供
///
pub fn read(_fd: usize, _base: *mut u8, _len: usize) -> isize {
    // // 同步接口
    // sys_read(fd, base, len)
    // 异步接口
    // sys_read_(fd, base, len).await
    // 中间如何与异步的 async 结合起来，如果外层的函数没有经过 async 关键字包装，那么如何实现这种异步的调用，只能使用原始的方式进行，并且在这里与异步运行时结合起来
    // 这里需要假设，所有的系统调用都通过同一个接口进入，根据不同的 id 分发至不同的处理流程。
    0
}

/// 这个是同步系统调用的接口
pub fn sys_read(_fd: usize, _base: *mut u8, _len: usize) -> isize {
    0
}

/// 这个是异步系统调用的接口
pub fn sys_read_(_fd: usize, _base: *mut u8, _len: usize) -> ReadFuture {
    ReadFuture { _fd, _base, _len }
}

use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

pub struct ReadFuture {
    _fd: usize,
    _base: *mut u8,
    _len: usize,
}

impl Future for ReadFuture {
    type Output = isize;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Ready(0)
    }
}
