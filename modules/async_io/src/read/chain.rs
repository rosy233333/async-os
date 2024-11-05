use core::fmt;
use core::pin::Pin;
use core::task::{Context, Poll};

use pin_project_lite::pin_project;

use crate::{self as io, AsyncBufRead, AsyncRead, IoSliceMut};

pin_project! {
    /// Adaptor to chain together two readers.
    ///
    /// This struct is generally created by calling [`chain`] on a reader.
    /// Please see the documentation of [`chain`] for more details.
    ///
    /// [`chain`]: trait.Read.html#method.chain
    pub struct Chain<T, U> {
        #[pin]
        pub(crate) first: T,
        #[pin]
        pub(crate) second: U,
        pub(crate) done_first: bool,
    }
}

impl<T, U> Chain<T, U> {
    /// Consumes the `Chain`, returning the wrapped readers.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # fn main() -> async_std::io::Result<()> { async_std::task::block_on(async {
    /// #
    /// use async_std::prelude::*;
    /// use async_std::fs::File;
    ///
    /// let foo_file = File::open("foo.txt").await?;
    /// let bar_file = File::open("bar.txt").await?;
    ///
    /// let chain = foo_file.chain(bar_file);
    /// let (foo_file, bar_file) = chain.into_inner();
    /// #
    /// # Ok(()) }) }
    /// ```
    pub fn into_inner(self) -> (T, U) {
        (self.first, self.second)
    }

    /// Gets references to the underlying readers in this `Chain`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # fn main() -> async_std::io::Result<()> { async_std::task::block_on(async {
    /// #
    /// use async_std::prelude::*;
    /// use async_std::fs::File;
    ///
    /// let foo_file = File::open("foo.txt").await?;
    /// let bar_file = File::open("bar.txt").await?;
    ///
    /// let chain = foo_file.chain(bar_file);
    /// let (foo_file, bar_file) = chain.get_ref();
    /// #
    /// # Ok(()) }) }
    /// ```
    pub fn get_ref(&self) -> (&T, &U) {
        (&self.first, &self.second)
    }

    /// Gets mutable references to the underlying readers in this `Chain`.
    ///
    /// Care should be taken to avoid modifying the internal I/O state of the
    /// underlying readers as doing so may corrupt the internal state of this
    /// `Chain`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # fn main() -> async_std::io::Result<()> { async_std::task::block_on(async {
    /// #
    /// use async_std::prelude::*;
    /// use async_std::fs::File;
    ///
    /// let foo_file = File::open("foo.txt").await?;
    /// let bar_file = File::open("bar.txt").await?;
    ///
    /// let mut chain = foo_file.chain(bar_file);
    /// let (foo_file, bar_file) = chain.get_mut();
    /// #
    /// # Ok(()) }) }
    /// ```
    pub fn get_mut(&mut self) -> (&mut T, &mut U) {
        (&mut self.first, &mut self.second)
    }
}

impl<T: fmt::Debug, U: fmt::Debug> fmt::Debug for Chain<T, U> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Chain")
            .field("t", &self.first)
            .field("u", &self.second)
            .finish()
    }
}

impl<T: AsyncRead, U: AsyncRead> AsyncRead for Chain<T, U> {
    fn read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<io::Result<usize>> {
        let this = self.project();
        if !*this.done_first {
            match core::task::ready!(this.first.read(cx, buf)) {
                Ok(0) if !buf.is_empty() => *this.done_first = true,
                Ok(n) => return Poll::Ready(Ok(n)),
                Err(err) => return Poll::Ready(Err(err)),
            }
        }

        this.second.read(cx, buf)
    }

    fn read_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &mut [IoSliceMut<'_>],
    ) -> Poll<io::Result<usize>> {
        let this = self.project();
        if !*this.done_first {
            match core::task::ready!(this.first.read_vectored(cx, bufs)) {
                Ok(0) if !bufs.is_empty() => *this.done_first = true,
                Ok(n) => return Poll::Ready(Ok(n)),
                Err(err) => return Poll::Ready(Err(err)),
            }
        }

        this.second.read_vectored(cx, bufs)
    }
}

impl<T: AsyncBufRead, U: AsyncBufRead> AsyncBufRead for Chain<T, U> {
    fn fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        let this = self.project();
        if !*this.done_first {
            match core::task::ready!(this.first.fill_buf(cx)) {
                Ok(buf) if buf.is_empty() => {
                    *this.done_first = true;
                }
                Ok(buf) => return Poll::Ready(Ok(buf)),
                Err(err) => return Poll::Ready(Err(err)),
            }
        }

        this.second.fill_buf(cx)
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        let this = self.project();
        if !*this.done_first {
            this.first.consume(amt)
        } else {
            this.second.consume(amt)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::read::Read;
    use crate::Cursor;
    use alloc::boxed::Box;
    use core::{
        future::Future,
        task::{Context, Waker},
    };

    #[test]
    fn test_chain_basics() {
        let source1 = Cursor::new(vec![0, 1, 2]);
        let source2 = Cursor::new(vec![3, 4, 5]);
        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);
        let fut = async move {
            let mut buffer = Vec::new();

            let mut source = source1.chain(source2);

            assert_eq!(6, source.read_to_end(&mut buffer).await.unwrap());
            assert_eq!(buffer, vec![0, 1, 2, 3, 4, 5]);
            println!("buffer {:?}", buffer);
        };
        let _ = Box::pin(fut).as_mut().poll(&mut cx);
    }
}
