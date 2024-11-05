use core::pin::Pin;
use core::task::{Context, Poll};

use pin_project_lite::pin_project;

use super::AsyncStream;

pin_project! {
    /// A stream to skip first n elements of another stream.
    ///
    /// This `struct` is created by the [`skip`] method on [`Stream`]. See its
    /// documentation for more.
    ///
    /// [`skip`]: trait.Stream.html#method.skip
    /// [`Stream`]: trait.Stream.html
    #[derive(Debug)]
    pub struct Skip<S> {
        #[pin]
        stream: S,
        n: usize,
    }
}

impl<S> Skip<S> {
    pub(crate) fn new(stream: S, n: usize) -> Self {
        Self { stream, n }
    }
}

impl<S> AsyncStream for Skip<S>
where
    S: AsyncStream,
{
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();
        loop {
            let next = core::task::ready!(this.stream.as_mut().poll_next(cx));

            match next {
                Some(v) => match *this.n {
                    0 => return Poll::Ready(Some(v)),
                    _ => *this.n -= 1,
                },
                None => return Poll::Ready(None),
            }
        }
    }
}
