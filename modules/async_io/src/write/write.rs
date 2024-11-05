use core::future::Future;
use core::pin::Pin;

use crate::{self as io, AsyncWrite};
use core::task::{Context, Poll};

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct WriteFuture<'a, T: Unpin + ?Sized> {
    pub(crate) writer: &'a mut T,
    pub(crate) buf: &'a [u8],
}

impl<T: AsyncWrite + Unpin + ?Sized> Future for WriteFuture<'_, T> {
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let buf = self.buf;
        Pin::new(&mut *self.writer).write(cx, buf)
    }
}
