use core::future::Future;
use core::pin::Pin;

use super::AsyncStream;
use core::task::{Context, Poll};

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct NextFuture<'a, T: Unpin + ?Sized> {
    pub(crate) stream: &'a mut T,
}

impl<T: AsyncStream + Unpin + ?Sized> Future for NextFuture<'_, T> {
    type Output = Option<T::Item>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut *self.stream).poll_next(cx)
    }
}
