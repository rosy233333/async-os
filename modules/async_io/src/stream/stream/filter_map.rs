use core::pin::Pin;
use core::task::{Context, Poll};

use pin_project_lite::pin_project;

use super::AsyncStream;

pin_project! {
    #[derive(Debug)]
    pub struct FilterMap<S, F> {
        #[pin]
        stream: S,
        f: F,
    }
}

impl<S, F> FilterMap<S, F> {
    pub(crate) fn new(stream: S, f: F) -> Self {
        Self { stream, f }
    }
}

impl<S, F, B> AsyncStream for FilterMap<S, F>
where
    S: AsyncStream,
    F: FnMut(S::Item) -> Option<B>,
{
    type Item = B;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        let next = core::task::ready!(this.stream.poll_next(cx));
        match next {
            Some(v) => match (this.f)(v) {
                Some(b) => Poll::Ready(Some(b)),
                None => Poll::Pending,
            },
            None => Poll::Ready(None),
        }
    }
}
