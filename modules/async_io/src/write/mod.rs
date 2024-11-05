mod flush;
mod write;
mod write_all;
mod write_fmt;
mod write_vectored;

use alloc::{boxed::Box, string::String, vec::Vec};
use axerrno::ax_err_type;
use flush::FlushFuture;
use write::WriteFuture;
use write_all::WriteAllFuture;
use write_fmt::WriteFmtFuture;
use write_vectored::WriteVectoredFuture;

use crate::{IoSlice, Result};
use core::{
    ops::DerefMut,
    pin::Pin,
    task::{Context, Poll},
};

/// 异步写
///
/// 类似于 `std::io::Write`，但集成了异步任务系统。
/// `write` 函数不同于 `std::io::Write::write`，
/// 会自动将当前任务放入等待队列，并让出 CPU
pub trait AsyncWrite {
    /// 尝试将 `buf` 中的数据写到对象中
    ///
    /// 一旦成功，则返回 `Poll::Ready(Ok(num_bytes_written))`
    ///
    /// 如果对象暂时不可写，将返回 `Poll::Pending`，并让出 CPU
    ///
    /// 当对象可写或关闭时，唤醒等待的任务
    ///
    /// # 实现
    ///
    /// 这个函数不会返回 `WouldBlock` 或 `Interrupted` 错误，
    /// 而是将这些错误转化为 `Poll::Pending`，并且在内部进行重试
    /// 或者转化为其他错误
    ///
    /// 如果该对象只能通过 `flush` 才可以变成可写的状态，
    /// 则 `write` 函数中必须尝试使用 `flush` 使对象可写
    fn write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>>;

    /// 尝试使用向量 IO 异步的将数据从 `bufs` 写到对象中
    ///
    /// 这个函数类似于 `write`，但允许在单个函数中将多个缓冲区的数据写到对象中
    ///
    /// 一旦成功，则返回 `Poll::Ready(Ok(num_bytes_written))`
    ///
    /// 如果对象不可写，则返回 `Poll::Pending`，当前任务让出 CPU
    ///
    /// 当对象可写或关闭时，唤醒等待的任务
    ///
    /// 默认情况下，这个函数对 `bufs` 中第一个非空的缓冲区使用 `write` 函数，
    /// 或者直接写取到空的缓冲区。支持向量 IO 的对象必须重写这个函数
    ///
    /// # 实现
    ///
    /// 这个函数不会返回 `WouldBlock` 或 `Interrupted` 错误，
    /// 而是将这些错误转化为 `Poll::Pending`，并且在内部进行重试
    /// 或者转化为其他错误
    fn write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<Result<usize>> {
        for b in bufs {
            if !b.is_empty() {
                return self.write(cx, b);
            }
        }

        self.write(cx, &[])
    }

    /// 尝试异步刷新对象，保证缓冲区的数据达到目标
    ///
    /// 一旦成功，返回 `Poll::Ready(OK(()))`
    ///
    /// 如果不能立即完成，则返回 `Poll::Pending`，当前 CPU 让权
    ///
    ///
    /// # 实现
    ///
    /// 这个函数不会返回 `WouldBlock` 或 `Interrupted` 错误，
    /// 而是将这些错误转化为 `Poll::Pending`，并且在内部进行重试
    /// 或者转化为其他错误
    ///
    /// 只有当缓冲区实际存在数据时，这个操作才有意义
    fn flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>>;

    /// 尝试关闭对象
    ///
    /// 一旦成功，返回 `Poll::Ready(Ok(()))`.
    ///
    /// 如果不能马上完成，则返回 `Poll::Pending`，当前任务让出 CPU
    ///
    /// # 实现
    ///
    /// 这个函数不会返回 `WouldBlock` 或 `Interrupted` 错误，
    /// 而是将这些错误转化为 `Poll::Pending`，并且在内部进行重试
    /// 或者转化为其他错误
    fn close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>>;
}

macro_rules! deref_async_write {
    () => {
        fn write(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<Result<usize>> {
            Pin::new(&mut **self).write(cx, buf)
        }

        fn write_vectored(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            bufs: &[IoSlice<'_>],
        ) -> Poll<Result<usize>> {
            Pin::new(&mut **self).write_vectored(cx, bufs)
        }

        fn flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
            Pin::new(&mut **self).flush(cx)
        }

        fn close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
            Pin::new(&mut **self).close(cx)
        }
    };
}

impl<T: ?Sized + AsyncWrite + Unpin> AsyncWrite for Box<T> {
    deref_async_write!();
}

impl<T: ?Sized + AsyncWrite + Unpin> AsyncWrite for &mut T {
    deref_async_write!();
}

impl<P> AsyncWrite for Pin<P>
where
    P: DerefMut + Unpin,
    P::Target: AsyncWrite,
{
    fn write(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        self.get_mut().as_mut().write(cx, buf)
    }

    fn write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<Result<usize>> {
        self.get_mut().as_mut().write_vectored(cx, bufs)
    }

    fn flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        self.get_mut().as_mut().flush(cx)
    }

    fn close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        self.get_mut().as_mut().close(cx)
    }
}

impl AsyncWrite for Vec<u8> {
    fn write(mut self: Pin<&mut Self>, _: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize>> {
        self.extend_from_slice(buf);
        Poll::Ready(Ok(buf.len()))
    }

    fn write_vectored(
        mut self: Pin<&mut Self>,
        _: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<Result<usize>> {
        let len = bufs.iter().map(|b| b.len()).sum();
        self.reserve(len);
        for buf in bufs {
            self.extend_from_slice(buf);
        }
        Poll::Ready(Ok(len))
    }

    fn flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<()>> {
        self.flush(cx)
    }
}

#[doc = r#"
    Extension methods for [`Write`].

    [`Write`]: ../trait.Write.html
"#]
pub trait Write: AsyncWrite {
    #[doc = r#"
        Writes some bytes into the byte stream.

        Returns the number of bytes written from the start of the buffer.

        If the return value is `Ok(n)` then it must be guaranteed that
        `0 <= n <= buf.len()`. A return value of `0` typically means that the underlying
        object is no longer able to accept bytes and will likely not be able to in the
        future as well, or that the buffer provided is empty.

        # Examples

        ```no_run
        # fn main() -> std::io::Result<()> { async_std::task::block_on(async {
        #
        use async_std::fs::File;
        use async_std::prelude::*;

        let mut file = File::create("a.txt").await?;

        let n = file.write(b"hello world").await?;
        #
        # Ok(()) }) }
        ```
    "#]
    fn write<'a>(&'a mut self, buf: &'a [u8]) -> WriteFuture<'a, Self>
    where
        Self: Unpin,
    {
        WriteFuture { writer: self, buf }
    }

    #[doc = r#"
        Flushes the stream to ensure that all buffered contents reach their destination.

        # Examples

        ```no_run
        # fn main() -> std::io::Result<()> { async_std::task::block_on(async {
        #
        use async_std::fs::File;
        use async_std::prelude::*;

        let mut file = File::create("a.txt").await?;

        file.write_all(b"hello world").await?;
        file.flush().await?;
        #
        # Ok(()) }) }
        ```
    "#]
    fn flush(&mut self) -> FlushFuture<'_, Self>
    where
        Self: Unpin,
    {
        FlushFuture { writer: self }
    }

    #[doc = r#"
        Like [`write`], except that it writes from a slice of buffers.

        Data is copied from each buffer in order, with the final buffer read from possibly
        being only partially consumed. This method must behave as a call to [`write`] with
        the buffers concatenated would.

        The default implementation calls [`write`] with either the first nonempty buffer
        provided, or an empty one if none exists.

        [`write`]: #tymethod.write
    "#]
    fn write_vectored<'a>(&'a mut self, bufs: &'a [IoSlice<'a>]) -> WriteVectoredFuture<'a, Self>
    where
        Self: Unpin,
    {
        WriteVectoredFuture { writer: self, bufs }
    }

    #[doc = r#"
        Writes an entire buffer into the byte stream.

        This method will continuously call [`write`] until there is no more data to be
        written or an error is returned. This method will not return until the entire
        buffer has been successfully written or such an error occurs.

        [`write`]: #tymethod.write

        # Examples

        ```no_run
        # fn main() -> std::io::Result<()> { async_std::task::block_on(async {
        #
        use async_std::fs::File;
        use async_std::prelude::*;

        let mut file = File::create("a.txt").await?;

        file.write_all(b"hello world").await?;
        #
        # Ok(()) }) }
        ```

        [`write`]: #tymethod.write
    "#]
    fn write_all<'a>(&'a mut self, buf: &'a [u8]) -> WriteAllFuture<'a, Self>
    where
        Self: Unpin,
    {
        WriteAllFuture { writer: self, buf }
    }

    #[doc = r#"
        Writes a formatted string into this writer, returning any error encountered.

        This method will continuously call [`write`] until there is no more data to be
        written or an error is returned. This future will not resolve until the entire
        buffer has been successfully written or such an error occurs.

        [`write`]: #tymethod.write

        # Examples

        ```no_run
        # fn main() -> std::io::Result<()> { async_std::task::block_on(async {
        #
        use async_std::io::prelude::*;
        use async_std::fs::File;

        let mut buffer = File::create("foo.txt").await?;

        // this call
        write!(buffer, "{:.*}", 2, 1.234567).await?;
        // turns into this:
        buffer.write_fmt(format_args!("{:.*}", 2, 1.234567)).await?;
        #
        # Ok(()) }) }
        ```
    "#]
    fn write_fmt<'a>(&'a mut self, fmt: core::fmt::Arguments<'_>) -> WriteFmtFuture<'a, Self>
    where
        Self: Unpin,
    {
        // In order to not have to implement an async version of `fmt` including private types
        // and all, we convert `Arguments` to a `Result<Vec<u8>>` and pass that to the Future.
        // Doing an owned conversion saves us from juggling references.
        let mut string = String::new();
        let res = core::fmt::write(&mut string, fmt)
            .map(|_| string.into_bytes())
            .map_err(|_| ax_err_type!(Unsupported, "formatter error"));
        WriteFmtFuture {
            writer: self,
            res: Some(res),
            buffer: None,
            amt: 0,
        }
    }
}

impl<T: AsyncWrite + ?Sized> Write for T {}
