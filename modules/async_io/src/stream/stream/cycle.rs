use core::pin::Pin;

use core::task::ready;
use pin_project_lite::pin_project;

use super::AsyncStream;
use core::task::{Context, Poll};

pin_project! {
    /// A stream that will repeatedly yield the same list of elements.
    #[derive(Debug)]
    pub struct Cycle<S> {
        orig: S,
        #[pin]
        source: S,
    }
}

impl<S> Cycle<S>
where
    S: AsyncStream + Clone,
{
    pub(crate) fn new(source: S) -> Self {
        Self {
            orig: source.clone(),
            source,
        }
    }
}

impl<S> AsyncStream for Cycle<S>
where
    S: AsyncStream + Clone,
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        match ready!(this.source.as_mut().poll_next(cx)) {
            None => {
                this.source.set(this.orig.clone());
                this.source.poll_next(cx)
            }
            item => Poll::Ready(item),
        }
    }
}
