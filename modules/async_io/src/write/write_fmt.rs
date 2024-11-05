use core::future::Future;
use core::pin::Pin;

use alloc::vec::Vec;

use crate::{self as io, AsyncWrite};
use core::task::{Context, Poll};

#[doc(hidden)]
#[allow(missing_debug_implementations)]
#[must_use]
pub struct WriteFmtFuture<'a, T: Unpin + ?Sized> {
    pub(crate) writer: &'a mut T,
    pub(crate) res: Option<io::Result<Vec<u8>>>,
    pub(crate) buffer: Option<Vec<u8>>,
    pub(crate) amt: usize,
}

impl<T: AsyncWrite + Unpin + ?Sized> Future for WriteFmtFuture<'_, T> {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Process the internal Result the first time we run.
        if self.buffer.is_none() {
            match self.res.take().unwrap() {
                Err(err) => return Poll::Ready(Err(err)),
                Ok(buffer) => self.buffer = Some(buffer),
            };
        }

        // Get the types from the future.
        let Self {
            writer,
            amt,
            buffer,
            ..
        } = &mut *self;
        let buffer = buffer.as_mut().unwrap();

        // Copy the data from the buffer into the writer until it's done.
        loop {
            if *amt == buffer.len() {
                core::task::ready!(Pin::new(&mut **writer).flush(cx))?;
                return Poll::Ready(Ok(()));
            }
            let i = core::task::ready!(Pin::new(&mut **writer).write(cx, &buffer[*amt..]))?;
            if i == 0 {
                return Poll::Ready(Err(io::Error::WriteZero));
            }
            *amt += i;
        }
    }
}
