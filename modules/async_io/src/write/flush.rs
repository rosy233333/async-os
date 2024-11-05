use core::future::Future;
use core::pin::Pin;

use crate::{self as io, AsyncWrite};
use core::task::{Context, Poll};

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct FlushFuture<'a, T: Unpin + ?Sized> {
    pub(crate) writer: &'a mut T,
}

impl<T: AsyncWrite + Unpin + ?Sized> Future for FlushFuture<'_, T> {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut *self.writer).flush(cx)
    }
}
