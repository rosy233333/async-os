//! 这里的实现，直接使用了 axhal 模块中的 console 的实现，
//! 这里无法读出数据时，会直接调用 cx.waker().wake_by_ref() 函数
//! 将任务重新放回到就绪队列中，定期轮询
//!
//! 正常的做法应该是等键盘输入产生了中断后才唤醒任务，将其放入就绪队列
//!
use super::{ax_console_read_byte, ax_console_write_bytes, Result};
use async_io::{AsyncRead, AsyncWrite, BufReader, Write};
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use lazy_init::LazyInit;
use sync::Mutex;

struct StdinRaw;
struct StdoutRaw;

impl AsyncRead for StdinRaw {
    fn read(self: Pin<&mut Self>, _cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<Result<usize>> {
        let mut read_len = 0;
        while read_len < buf.len() {
            if let Some(c) = ax_console_read_byte() {
                buf[read_len] = c;
                read_len += 1;
            } else {
                break;
            }
        }
        Poll::Ready(Ok(read_len))
    }
}

impl AsyncWrite for StdoutRaw {
    fn write(self: Pin<&mut Self>, _cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        Poll::Ready(ax_console_write_bytes(buf))
    }

    fn flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        self.flush(cx)
    }
}

/// A handle to the standard input stream of a process.
pub struct Stdin {
    inner: &'static Mutex<BufReader<StdinRaw>>,
}

impl AsyncRead for Stdin {
    fn read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<Result<usize>> {
        let mut lock = futures_core::ready!(Pin::new(&mut self.inner.lock()).poll(cx));
        let read_len = futures_core::ready!(AsyncRead::read(Pin::new(&mut *lock), cx, buf))?;
        if buf.is_empty() || read_len > 0 {
            Poll::Ready(Ok(read_len))
        } else {
            drop(lock);
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

/// A handle to the global standard output stream of the current process.
pub struct Stdout {
    inner: &'static Mutex<StdoutRaw>,
}

impl AsyncWrite for Stdout {
    fn write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<async_io::Result<usize>> {
        let mut locked_inner = futures_core::ready!(Pin::new(&mut self.inner.lock()).poll(cx));
        AsyncWrite::write(Pin::new(&mut *locked_inner), cx, buf)
    }

    fn flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<async_io::Result<()>> {
        let mut locked_inner = futures_core::ready!(Pin::new(&mut self.inner.lock()).poll(cx));
        AsyncWrite::flush(Pin::new(&mut *locked_inner), cx)
    }

    fn close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<async_io::Result<()>> {
        let mut locked_inner = futures_core::ready!(Pin::new(&mut self.inner.lock()).poll(cx));
        AsyncWrite::close(Pin::new(&mut *locked_inner), cx)
    }
}

static INSTANCE: LazyInit<Mutex<BufReader<StdinRaw>>> = LazyInit::new();

/// Constructs a new handle to the standard input of the current process.
pub fn stdin() -> Stdin {
    if !INSTANCE.is_init() {
        INSTANCE.init_by(Mutex::new(BufReader::new(StdinRaw)));
    }
    Stdin { inner: &INSTANCE }
}

/// Constructs a new handle to the standard output of the current process.
pub fn stdout() -> Stdout {
    static INSTANCE: Mutex<StdoutRaw> = Mutex::new(StdoutRaw);
    Stdout { inner: &INSTANCE }
}

#[doc(hidden)]
pub async fn __print_impl(args: core::fmt::Arguments<'_>) {
    stdout().write_fmt(args).await.unwrap();
}
