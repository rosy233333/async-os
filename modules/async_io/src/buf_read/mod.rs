use crate::{self as io, AsyncRead, Result};
use alloc::{boxed::Box, string::String, vec::Vec};
use core::{
    mem,
    ops::DerefMut,
    pin::Pin,
    task::{Context, Poll},
};
use line::Lines;
use read_line::ReadLineFuture;
use read_until::ReadUntilFuture;
use split::Split;

mod line;
mod read_line;
mod read_until;
mod split;

/// 异步读
///
/// 类似于 `std::io::BufRead`，但集成了异步任务系统
/// `poll_fill_buf` 不同于 `std::io::BufRead::fill_buf`，
/// 当数据没有准备好时，当前任务主动让出 CPU
pub trait AsyncBufRead: AsyncRead {
    /// 尝试返回内部缓冲区的内容，如果缓冲区为空，则用内部读取器中的更多数据填充它。
    ///
    /// 一旦成功，返回 `Poll::Ready(Ok(buf))`
    ///
    /// 如果数据没有准备好，返回 `Poll::Pending`，当前任务让出 CPU
    /// 当对象可读或者关闭时，唤醒等待的任务
    ///
    /// 此函数是低级调用。它需要与 [`consume`] 方法配合使用才能正常运行。
    /// 调用此方法时，不会“读取”任何内容，因为稍后调用 [`read`] 可能会返回相同的内容。
    /// 因此，必须使用从此缓冲区消耗的字节数来调用 [`consume`]，以确保字节不会重复返回。
    ///
    /// [`read`]: AsyncRead::read
    /// [`consume`]: BufRead::consume
    ///
    /// 返回空缓冲区表明流已到达 EOF
    ///
    /// # 实现
    ///
    /// 这个函数不会返回 `WouldBlock` 或 `Interrupted` 错误，
    /// 而是将这些错误转化为 `Poll::Pending`，并且在内部进行重试
    /// 或者转化为其他错误
    fn fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<&[u8]>>;

    /// 告诉这个缓冲区，缓冲区中的“amt”字节已经被消耗，
    /// 因此它们不再应在对[“read”]的调用中返回。
    ///
    /// 此函数是低级调用。它需要与 [`fill_buf`] 方法配对才能正常运行。
    /// 此函数不执行任何 I/O，它只是通知此对象，
    /// 从 [`fill_buf`] 返回的一定量的缓冲区已被使用，不应再返回。
    /// 因此，如果在调用此函数之前未调用 [`fill_buf`]，则此函数可能会执行奇怪的操作。
    ///
    /// `amt` 必须 `<=` [`fill_buf`] 返回的缓冲区中的字节数。
    ///
    /// [`read`]: AsyncRead::poll_read
    /// [`fill_buf`]: BufRead::fill_buf
    fn consume(self: Pin<&mut Self>, amt: usize);
}

impl AsyncBufRead for &[u8] {
    #[inline]
    fn fill_buf(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<&[u8]>> {
        Poll::Ready(Ok(*self))
    }

    #[inline]
    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        *self = &self[amt..];
    }
}

macro_rules! deref_async_buf_read {
    () => {
        fn fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<&[u8]>> {
            Pin::new(&mut **self.get_mut()).fill_buf(cx)
        }

        fn consume(mut self: Pin<&mut Self>, amt: usize) {
            Pin::new(&mut **self).consume(amt)
        }
    };
}

impl<T: ?Sized + AsyncBufRead + Unpin> AsyncBufRead for Box<T> {
    deref_async_buf_read!();
}

impl<T: ?Sized + AsyncBufRead + Unpin> AsyncBufRead for &mut T {
    deref_async_buf_read!();
}

impl<P> AsyncBufRead for Pin<P>
where
    P: DerefMut + Unpin,
    P::Target: AsyncBufRead,
{
    fn fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<&[u8]>> {
        self.get_mut().as_mut().fill_buf(cx)
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        self.get_mut().as_mut().consume(amt)
    }
}

#[doc = r#"
    Extension methods for [`BufRead`].

    [`BufRead`]: ../trait.BufRead.html
"#]
pub trait BufRead: AsyncBufRead {
    #[doc = r#"
        Reads all bytes into `buf` until the delimiter `byte` or EOF is reached.

        This function will read bytes from the underlying stream until the delimiter or EOF
        is found. Once found, all bytes up to, and including, the delimiter (if found) will
        be appended to `buf`.

        If successful, this function will return the total number of bytes read.

        # Examples

        ```no_run
        # fn main() -> std::io::Result<()> { async_std::task::block_on(async {
        #
        use async_std::fs::File;
        use async_std::io::BufReader;
        use async_std::prelude::*;

        let mut file = BufReader::new(File::open("a.txt").await?);

        let mut buf = Vec::with_capacity(1024);
        let n = file.read_until(b'\n', &mut buf).await?;
        #
        # Ok(()) }) }
        ```

        Multiple successful calls to `read_until` append all bytes up to and including to
        `buf`:
        ```
        # fn main() -> std::io::Result<()> { async_std::task::block_on(async {
        #
        use async_std::io::BufReader;
        use async_std::prelude::*;

        let from: &[u8] = b"append\nexample\n";
        let mut reader = BufReader::new(from);
        let mut buf = vec![];

        let mut size = reader.read_until(b'\n', &mut buf).await?;
        assert_eq!(size, 7);
        assert_eq!(buf, b"append\n");

        size += reader.read_until(b'\n', &mut buf).await?;
        assert_eq!(size, from.len());

        assert_eq!(buf, from);
        #
        # Ok(()) }) }
        ```
    "#]
    fn read_until<'a>(&'a mut self, byte: u8, buf: &'a mut Vec<u8>) -> ReadUntilFuture<'a, Self>
    where
        Self: Unpin,
    {
        ReadUntilFuture {
            reader: self,
            byte,
            buf,
            read: 0,
        }
    }

    #[doc = r#"
        Reads all bytes and appends them into `buf` until a newline (the 0xA byte) is
        reached.

        This function will read bytes from the underlying stream until the newline
        delimiter (the 0xA byte) or EOF is found. Once found, all bytes up to, and
        including, the delimiter (if found) will be appended to `buf`.

        If successful, this function will return the total number of bytes read.

        If this function returns `Ok(0)`, the stream has reached EOF.

        # Errors

        This function has the same error semantics as [`read_until`] and will also return
        an error if the read bytes are not valid UTF-8. If an I/O error is encountered then
        `buf` may contain some bytes already read in the event that all data read so far
        was valid UTF-8.

        [`read_until`]: #method.read_until

        # Examples

        ```no_run
        # fn main() -> std::io::Result<()> { async_std::task::block_on(async {
        #
        use async_std::fs::File;
        use async_std::io::BufReader;
        use async_std::prelude::*;

        let mut file = BufReader::new(File::open("a.txt").await?);

        let mut buf = String::new();
        file.read_line(&mut buf).await?;
        #
        # Ok(()) }) }
        ```
    "#]
    fn read_line<'a>(&'a mut self, buf: &'a mut String) -> ReadLineFuture<'a, Self>
    where
        Self: Unpin,
    {
        ReadLineFuture {
            reader: self,
            bytes: unsafe { mem::replace(buf.as_mut_vec(), Vec::new()) },
            buf,
            read: 0,
        }
    }

    #[doc = r#"
        Returns a stream over the lines of this byte stream.

        The stream returned from this function will yield instances of
        [`io::Result`]`<`[`String`]`>`. Each string returned will *not* have a newline byte
        (the 0xA byte) or CRLF (0xD, 0xA bytes) at the end.

        [`io::Result`]: type.Result.html
        [`String`]: https://doc.rust-lang.org/std/string/struct.String.html

        # Examples

        ```no_run
        # fn main() -> std::io::Result<()> { async_std::task::block_on(async {
        #
        use async_std::fs::File;
        use async_std::io::BufReader;
        use async_std::prelude::*;

        let file = File::open("a.txt").await?;
        let mut lines = BufReader::new(file).lines();
        let mut count = 0;

        while let Some(line) = lines.next().await {
            line?;
            count += 1;
        }
        #
        # Ok(()) }) }
        ```
    "#]
    fn lines(self) -> Lines<Self>
    where
        Self: Unpin + Sized,
    {
        Lines {
            reader: self,
            buf: String::new(),
            bytes: Vec::new(),
            read: 0,
        }
    }

    #[doc = r#"
        Returns a stream over the contents of this reader split on the byte `byte`.

        The stream returned from this function will return instances of
        [`io::Result`]`<`[`Vec<u8>`]`>`. Each vector returned will *not* have
        the delimiter byte at the end.

        This function will yield errors whenever [`read_until`] would have
        also yielded an error.

        [`io::Result`]: type.Result.html
        [`Vec<u8>`]: ../vec/struct.Vec.html
        [`read_until`]: #method.read_until

        # Examples

        [`std::io::Cursor`][`Cursor`] is a type that implements `BufRead`. In
        this example, we use [`Cursor`] to iterate over all hyphen delimited
        segments in a byte slice

        [`Cursor`]: struct.Cursor.html

        ```
        # fn main() -> std::io::Result<()> { async_std::task::block_on(async {
        #
        use async_std::prelude::*;
        use async_std::io;

        let cursor = io::Cursor::new(b"lorem-ipsum-dolor");

        let mut split_iter = cursor.split(b'-').map(|l| l.unwrap());
        assert_eq!(split_iter.next().await, Some(b"lorem".to_vec()));
        assert_eq!(split_iter.next().await, Some(b"ipsum".to_vec()));
        assert_eq!(split_iter.next().await, Some(b"dolor".to_vec()));
        assert_eq!(split_iter.next().await, None);
        #
        # Ok(()) }) }
        ```
    "#]
    fn split(self, byte: u8) -> Split<Self>
    where
        Self: Sized,
    {
        Split {
            reader: self,
            buf: Vec::new(),
            delim: byte,
            read: 0,
        }
    }
}

impl<T: AsyncBufRead + ?Sized> BufRead for T {}

pub(crate) fn read_until_internal<R: BufRead + ?Sized>(
    mut reader: Pin<&mut R>,
    cx: &mut Context<'_>,
    byte: u8,
    buf: &mut Vec<u8>,
    read: &mut usize,
) -> Poll<io::Result<usize>> {
    loop {
        let (done, used) = {
            let available = core::task::ready!(reader.as_mut().fill_buf(cx))?;
            if let Some(i) = memchr::memchr(byte, available) {
                buf.extend_from_slice(&available[..=i]);
                (true, i + 1)
            } else {
                buf.extend_from_slice(available);
                (false, available.len())
            }
        };
        reader.as_mut().consume(used);
        *read += used;
        if done || used == 0 {
            return Poll::Ready(Ok(mem::replace(read, 0)));
        }
    }
}
