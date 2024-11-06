mod bytes;
mod chain;
mod read;
mod read_exact;
mod read_to_end;
mod read_to_string;
mod read_vectored;
mod take;

use read::ReadFuture;
use read_exact::ReadExactFuture;
use read_to_end::{read_to_end_internal, ReadToEndFuture};
use read_to_string::ReadToStringFuture;
use read_vectored::ReadVectoredFuture;

use core::mem;

pub use bytes::Bytes;
pub use chain::Chain;
pub use take::Take;

use crate::{IoSliceMut, Result};
use alloc::{boxed::Box, string::String, vec::Vec};
use core::{
    cmp,
    ops::DerefMut,
    pin::Pin,
    task::{Context, Poll},
};

/// 异步读接口
///
/// 类似于 std::io::Read，但与异步任务系统集成。
/// read 方法不同于 std::io::Read::read，当数据还没有准备好时，
/// 会自动将当前任务放入等待队列，并让出 CPU
pub trait AsyncRead {
    /// 尝试异步的将数据读取到 buf 中
    ///
    /// 一旦成功，则会返回 `Poll::Ready(Ok(num_bytes_read))`
    ///
    /// 如果没有数据，则会返回 `Poll::Pending`，当前任务让出 CPU
    ///
    /// 当对象变得可读或者关闭时，唤醒等待的任务
    ///
    /// # 实现
    ///
    /// 这个函数不会返回 `WouldBlock` 或 `Interrupted` 错误，
    /// 而是将这些错误转化为 `Poll::Pending`，并且在内部进行重试
    /// 或者转化为其他错误
    fn read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<Result<usize>>;

    /// 尝试异步的将数据读取到 `bufs`
    ///
    /// 类似于 `read`，但允许在单个函数中将数据读取到多个缓冲区中
    ///
    /// 一旦成功，则返回 `Poll::Ready(Ok(num_bytes_read))`
    ///
    /// 如果没有数据，则会返回 `Poll::Pending`，当前任务让出 CPU
    ///
    /// 当对象变得可读或者关闭时，唤醒等待的任务
    ///
    /// 默认情况下，这个函数对 `bufs` 中第一个非空的缓冲区使用 `read` 函数，
    /// 或者直接读取到空的缓冲区。支持向量 IO 的对象必须重写这个函数
    ///
    /// # 实现
    ///
    /// 这个函数不会返回 `WouldBlock` 或 `Interrupted` 错误，
    /// 而是将这些错误转化为 `Poll::Pending`，并且在内部进行重试
    /// 或者转化为其他错误
    fn read_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<Result<usize>> {
        for b in bufs {
            if !b.is_empty() {
                return self.read(cx, b);
            }
        }

        self.read(cx, &mut [])
    }
}

macro_rules! deref_async_read {
    () => {
        fn read(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            buf: &mut [u8],
        ) -> Poll<Result<usize>> {
            Pin::new(&mut **self).read(cx, buf)
        }

        fn read_vectored(
            mut self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            bufs: &mut [IoSliceMut<'_>],
        ) -> Poll<Result<usize>> {
            Pin::new(&mut **self).read_vectored(cx, bufs)
        }
    };
}

impl<T: ?Sized + AsyncRead + Unpin> AsyncRead for Box<T> {
    deref_async_read!();
}

impl<T: ?Sized + AsyncRead + Unpin> AsyncRead for &mut T {
    deref_async_read!();
}

impl<P> AsyncRead for Pin<P>
where
    P: DerefMut + Unpin,
    P::Target: AsyncRead,
{
    fn read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<Result<usize>> {
        self.get_mut().as_mut().read(cx, buf)
    }

    fn read_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<Result<usize>> {
        self.get_mut().as_mut().read_vectored(cx, bufs)
    }
}

impl AsyncRead for &[u8] {
    // delegate_async_read_to_stdio!();
    #[inline]
    fn read(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize>> {
        let amt = cmp::min(buf.len(), self.len());
        let (a, b) = self.split_at(amt);

        // 首先检查我们要读取的字节数是否很小：
        // `copy_from_slice` 通常会扩展为对 `memcpy` 的调用，并且对于单个字节来说，开销很大。
        //
        if amt == 1 {
            buf[0] = a[0];
        } else {
            buf[..amt].copy_from_slice(a);
        }
        *self = b;
        // log::error!("async read for u8 {}", amt);
        Poll::Ready(Ok(amt))
    }

    #[inline]
    fn read_vectored(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<Result<usize>> {
        let mut nread = 0;
        for buf in bufs {
            nread += core::task::ready!(Pin::new(&mut *self).read(_cx, buf))?;
            if self.is_empty() {
                break;
            }
        }
        Poll::Ready(Ok(nread))
    }
}

#[doc = r#"
    Extension methods for [`Read`].

    [`Read`]: ../trait.Read.html
"#]
pub trait Read: AsyncRead {
    #[doc = r#"
        Reads some bytes from the byte stream.

        Returns the number of bytes read from the start of the buffer.

        If the return value is `Ok(n)`, then it must be guaranteed that
        `0 <= n <= buf.len()`. A nonzero `n` value indicates that the buffer has been
        filled in with `n` bytes of data. If `n` is `0`, then it can indicate one of two
        scenarios:

        1. This reader has reached its "end of file" and will likely no longer be able to
           produce bytes. Note that this does not mean that the reader will always no
           longer be able to produce bytes.
        2. The buffer specified was 0 bytes in length.

        # Examples

        ```no_run
        # fn main() -> std::io::Result<()> { async_std::task::block_on(async {
        #
        use async_std::fs::File;
        use async_std::prelude::*;

        let mut file = File::open("a.txt").await?;

        let mut buf = vec![0; 1024];
        let n = file.read(&mut buf).await?;
        #
        # Ok(()) }) }
        ```
    "#]
    fn read<'a>(&'a mut self, buf: &'a mut [u8]) -> ReadFuture<'a, Self>
    where
        Self: Unpin,
    {
        ReadFuture { reader: self, buf }
    }

    #[doc = r#"
        Like [`read`], except that it reads into a slice of buffers.

        Data is copied to fill each buffer in order, with the final buffer written to
        possibly being only partially filled. This method must behave as a single call to
        [`read`] with the buffers concatenated would.

        The default implementation calls [`read`] with either the first nonempty buffer
        provided, or an empty one if none exists.

        [`read`]: #tymethod.read
    "#]
    fn read_vectored<'a>(
        &'a mut self,
        bufs: &'a mut [IoSliceMut<'a>],
    ) -> ReadVectoredFuture<'a, Self>
    where
        Self: Unpin,
    {
        ReadVectoredFuture { reader: self, bufs }
    }

    #[doc = r#"
        Reads all bytes from the byte stream.

        All bytes read from this stream will be appended to the specified buffer `buf`.
        This function will continuously call [`read`] to append more data to `buf` until
        [`read`] returns either `Ok(0)` or an error.

        If successful, this function will return the total number of bytes read.

        [`read`]: #tymethod.read

        # Examples

        ```no_run
        # fn main() -> std::io::Result<()> { async_std::task::block_on(async {
        #
        use async_std::fs::File;
        use async_std::prelude::*;

        let mut file = File::open("a.txt").await?;

        let mut buf = Vec::new();
        file.read_to_end(&mut buf).await?;
        #
        # Ok(()) }) }
        ```
    "#]
    fn read_to_end<'a>(&'a mut self, buf: &'a mut Vec<u8>) -> ReadToEndFuture<'a, Self>
    where
        Self: Unpin,
    {
        let start_len = buf.len();
        ReadToEndFuture {
            reader: self,
            buf,
            start_len,
        }
    }

    #[doc = r#"
        Reads all bytes from the byte stream and appends them into a string.

        If successful, this function will return the number of bytes read.

        If the data in this stream is not valid UTF-8 then an error will be returned and
        `buf` will be left unmodified.

        # Examples

        ```no_run
        # fn main() -> std::io::Result<()> { async_std::task::block_on(async {
        #
        use async_std::fs::File;
        use async_std::prelude::*;

        let mut file = File::open("a.txt").await?;

        let mut buf = String::new();
        file.read_to_string(&mut buf).await?;
        #
        # Ok(()) }) }
        ```
    "#]
    fn read_to_string<'a>(&'a mut self, buf: &'a mut String) -> ReadToStringFuture<'a, Self>
    where
        Self: Unpin,
    {
        let start_len = buf.len();
        ReadToStringFuture {
            reader: self,
            bytes: unsafe { mem::replace(buf.as_mut_vec(), Vec::new()) },
            buf,
            start_len,
        }
    }

    #[doc = r#"
        Reads the exact number of bytes required to fill `buf`.

        This function reads as many bytes as necessary to completely fill the specified
        buffer `buf`.

        No guarantees are provided about the contents of `buf` when this function is
        called, implementations cannot rely on any property of the contents of `buf` being
        true. It is recommended that implementations only write data to `buf` instead of
        reading its contents.

        If this function encounters an "end of file" before completely filling the buffer,
        it returns an error of the kind [`ErrorKind::UnexpectedEof`].  The contents of
        `buf` are unspecified in this case.

        If any other read error is encountered then this function immediately returns. The
        contents of `buf` are unspecified in this case.

        If this function returns an error, it is unspecified how many bytes it has read,
        but it will never read more than would be necessary to completely fill the buffer.

        [`ErrorKind::UnexpectedEof`]: enum.ErrorKind.html#variant.UnexpectedEof

        # Examples

        ```no_run
        # fn main() -> std::io::Result<()> { async_std::task::block_on(async {
        #
        use async_std::fs::File;
        use async_std::prelude::*;

        let mut file = File::open("a.txt").await?;

        let mut buf = vec![0; 10];
        file.read_exact(&mut buf).await?;
        #
        # Ok(()) }) }
        ```
    "#]
    fn read_exact<'a>(&'a mut self, buf: &'a mut [u8]) -> ReadExactFuture<'a, Self>
    where
        Self: Unpin,
    {
        ReadExactFuture { reader: self, buf }
    }

    #[doc = r#"
        Creates an adaptor which will read at most `limit` bytes from it.

        This function returns a new instance of `Read` which will read at most
        `limit` bytes, after which it will always return EOF ([`Ok(0)`]). Any
        read errors will not count towards the number of bytes read and future
        calls to [`read`] may succeed.

        # Examples

        [`File`]s implement `Read`:

        [`File`]: ../fs/struct.File.html
        [`Ok(0)`]: ../../std/result/enum.Result.html#variant.Ok
        [`read`]: tymethod.read

        ```no_run
        # fn main() -> std::io::Result<()> { async_std::task::block_on(async {
        #
        use async_std::io::prelude::*;
        use async_std::fs::File;

        let f = File::open("foo.txt").await?;
        let mut buffer = [0; 5];

        // read at most five bytes
        let mut handle = f.take(5);

        handle.read(&mut buffer).await?;
        #
        # Ok(()) }) }
        ```
    "#]
    fn take(self, limit: u64) -> Take<Self>
    where
        Self: Sized,
    {
        Take { inner: self, limit }
    }

    #[doc = r#"
        Creates a "by reference" adaptor for this instance of `Read`.

        The returned adaptor also implements `Read` and will simply borrow this
        current reader.

        # Examples

        [`File`][file]s implement `Read`:

        [file]: ../fs/struct.File.html

        ```no_run
        # fn main() -> std::io::Result<()> { async_std::task::block_on(async {
        #
        use async_std::prelude::*;
        use async_std::fs::File;

        let mut f = File::open("foo.txt").await?;
        let mut buffer = Vec::new();
        let mut other_buffer = Vec::new();

        {
            let reference = f.by_ref();

            // read at most 5 bytes
            reference.take(5).read_to_end(&mut buffer).await?;

        } // drop our &mut reference so we can use f again

        // original file still usable, read the rest
        f.read_to_end(&mut other_buffer).await?;
        #
        # Ok(()) }) }
        ```
    "#]
    fn by_ref(&mut self) -> &mut Self
    where
        Self: Sized,
    {
        self
    }

    #[doc = r#"
        Transforms this `Read` instance to a `Stream` over its bytes.

        The returned type implements `Stream` where the `Item` is
        `Result<u8, io::Error>`.
        The yielded item is `Ok` if a byte was successfully read and `Err`
        otherwise. EOF is mapped to returning `None` from this iterator.

        # Examples

        [`File`][file]s implement `Read`:

        [file]: ../fs/struct.File.html

        ```no_run
        # fn main() -> std::io::Result<()> { async_std::task::block_on(async {
        #
        use async_std::prelude::*;
        use async_std::fs::File;

        let f = File::open("foo.txt").await?;
        let mut s = f.bytes();

        while let Some(byte) = s.next().await {
            println!("{}", byte.unwrap());
        }
        #
        # Ok(()) }) }
        ```
    "#]
    fn bytes(self) -> Bytes<Self>
    where
        Self: Sized,
    {
        Bytes { inner: self }
    }

    #[doc = r#"
        Creates an adaptor which will chain this stream with another.

        The returned `Read` instance will first read all bytes from this object
        until EOF is encountered. Afterwards the output is equivalent to the
        output of `next`.

        # Examples

        [`File`][file]s implement `Read`:

        [file]: ../fs/struct.File.html

        ```no_run
        # fn main() -> std::io::Result<()> { async_std::task::block_on(async {
        #
        use async_std::prelude::*;
        use async_std::fs::File;

        let f1 = File::open("foo.txt").await?;
        let f2 = File::open("bar.txt").await?;

        let mut handle = f1.chain(f2);
        let mut buffer = String::new();

        // read the value into a String. We could use any Read method here,
        // this is just one example.
        handle.read_to_string(&mut buffer).await?;
        #
        # Ok(()) }) }
        ```
    "#]
    fn chain<R: Read>(self, next: R) -> Chain<Self, R>
    where
        Self: Sized,
    {
        Chain {
            first: self,
            second: next,
            done_first: false,
        }
    }
}

impl<T: AsyncRead + ?Sized> Read for T {}

// /// Initializes a buffer if necessary.
// ///
// /// Currently, a buffer is always initialized because `read_initializer`
// /// feature is not stable.
// #[inline]
// unsafe fn initialize<R: AsyncRead>(_reader: &R, buf: &mut [u8]) {
//     core::ptr::write_bytes(buf.as_mut_ptr(), 0, buf.len())
// }

#[cfg(test)]
mod tests {
    use crate::read::Read;
    use crate::Cursor;
    use alloc::boxed::Box;
    use core::future::Future;
    use core::task::{Context, Waker};

    #[test]
    fn test_read_to_end() {
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);
        let fut = async {
            let mut f = Cursor::new(vec![0u8, 1, 2, 3, 4, 5, 6, 7, 8]);
            let mut buffer = Vec::new();
            let mut other_buffer = Vec::new();

            {
                let reference = f.by_ref();

                // read at most 5 bytes
                assert_eq!(reference.take(5).read_to_end(&mut buffer).await.unwrap(), 5);
                for i in &buffer {
                    println!("buffer: {}", i);
                }
                assert_eq!(&buffer, &[0, 1, 2, 3, 4])
            } // drop our &mut reference so we can use f again

            // original file still usable, read the rest
            assert_eq!(f.read_to_end(&mut other_buffer).await.unwrap(), 4);
            assert_eq!(&other_buffer, &[5, 6, 7, 8]);
            for i in &other_buffer {
                println!("other_buffer: {}", i);
            }
        };
        let _ = Box::pin(fut).as_mut().poll(&mut cx);
    }

    #[test]
    fn test_read_exact() {
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);
        let fut = async {
            let mut f = Cursor::new(vec![0u8, 1, 2, 3, 4, 5, 6, 7, 8]);
            let mut buffer = vec![0u8; 9];
            assert_eq!(f.read_exact(&mut buffer).await.unwrap(), ());
            assert_eq!(&buffer, &[0, 1, 2, 3, 4, 5, 6, 7, 8]);
            assert_eq!(f.get_mut(), &[0, 1, 2, 3, 4, 5, 6, 7, 8]);
            for i in &buffer {
                println!("buffer: {}", i);
            }
        };
        let _ = Box::pin(fut).as_mut().poll(&mut cx);
    }

    #[test]
    fn test_read_to_string() {
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);
        let fut = async {
            let mut f = Cursor::new(String::from("hello, world"));
            let mut buffer = String::new();
            assert_eq!(f.read_to_string(&mut buffer).await.unwrap(), 12);
            assert_eq!(&buffer, "hello, world");
            println!("buffer: {}", buffer);
        };
        let _ = Box::pin(fut).as_mut().poll(&mut cx);
    }

    #[test]
    fn test_read_vectored() {
        use crate::IoSliceMut;
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);
        let fut = async {
            let mut f = Cursor::new(vec![0u8, 1, 2, 3, 4, 5, 6, 7, 8]);
            let mut a = vec![0u8; 3];
            let mut b = vec![0u8; 5];
            let mut c = vec![0u8; 1];
            let mut buffers = vec![
                IoSliceMut::new(&mut a),
                IoSliceMut::new(&mut b),
                IoSliceMut::new(&mut c),
            ];
            assert_eq!(f.read_vectored(&mut buffers).await.unwrap(), 9);
            println!("buffera: {:?}", a);
            println!("bufferb: {:?}", b);
            println!("bufferc: {:?}", c);
        };
        let _ = Box::pin(fut).as_mut().poll(&mut cx);
    }
}
