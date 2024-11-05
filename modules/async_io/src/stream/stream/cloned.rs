use super::AsyncStream;
use core::pin::Pin;
use core::task::{Context, Poll};
use pin_project_lite::pin_project;

pin_project! {
    /// A stream that clones the elements of an underlying stream.
    #[derive(Debug)]
    pub struct Cloned<S> {
        #[pin]
        stream: S,
    }
}

impl<S> Cloned<S> {
    pub(super) fn new(stream: S) -> Self {
        Self { stream }
    }
}

impl<'a, S, T: 'a> AsyncStream for Cloned<S>
where
    S: AsyncStream<Item = &'a T>,
    T: Clone,
{
    type Item = T;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        let next = futures_core::ready!(this.stream.poll_next(cx));
        Poll::Ready(next.cloned())
    }
}
