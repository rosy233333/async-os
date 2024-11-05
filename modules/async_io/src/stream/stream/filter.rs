use core::pin::Pin;

use pin_project_lite::pin_project;

use super::AsyncStream;
use core::task::{Context, Poll};

pin_project! {
    /// A stream to filter elements of another stream with a predicate.
    ///
    /// This `struct` is created by the [`filter`] method on [`Stream`]. See its
    /// documentation for more.
    ///
    /// [`filter`]: trait.Stream.html#method.filter
    /// [`Stream`]: trait.Stream.html
    #[derive(Debug)]
    pub struct Filter<S, P> {
        #[pin]
        stream: S,
        predicate: P,
    }
}

impl<S, P> Filter<S, P> {
    pub(super) fn new(stream: S, predicate: P) -> Self {
        Self { stream, predicate }
    }
}

impl<S, P> AsyncStream for Filter<S, P>
where
    S: AsyncStream,
    P: FnMut(&S::Item) -> bool,
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        let next = core::task::ready!(this.stream.poll_next(cx));

        match next {
            Some(v) if (this.predicate)(&v) => Poll::Ready(Some(v)),
            Some(_) => {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            None => Poll::Ready(None),
        }
    }
}
