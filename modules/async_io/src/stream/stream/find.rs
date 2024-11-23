use core::future::Future;
use core::pin::Pin;

use super::AsyncStream;
use core::task::{Context, Poll};

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct FindFuture<'a, S, P> {
    stream: &'a mut S,
    p: P,
}

impl<'a, S, P> FindFuture<'a, S, P> {
    pub(super) fn new(stream: &'a mut S, p: P) -> Self {
        Self { stream, p }
    }
}

impl<S: Unpin, P> Unpin for FindFuture<'_, S, P> {}

impl<'a, S, P> Future for FindFuture<'a, S, P>
where
    S: AsyncStream + Unpin + Sized,
    P: FnMut(&S::Item) -> bool,
{
    type Output = Option<S::Item>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let item = core::task::ready!(Pin::new(&mut *self.stream).poll_next(cx));

        match item {
            Some(v) if (&mut self.p)(&v) => Poll::Ready(Some(v)),
            Some(_) => Poll::Pending,
            None => Poll::Ready(None),
        }
    }
}
