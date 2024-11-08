use core::future::Future;
use core::pin::Pin;

use pin_project_lite::pin_project;

use super::AsyncStream;
use core::task::{Context, Poll};

pin_project! {
    #[doc(hidden)]
    pub struct CountFuture<S> {
        #[pin]
        stream: S,
        count: usize,
    }
}

impl<S> CountFuture<S> {
    pub(crate) fn new(stream: S) -> Self {
        Self { stream, count: 0 }
    }
}

impl<S> Future for CountFuture<S>
where
    S: AsyncStream,
{
    type Output = usize;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let next = core::task::ready!(this.stream.poll_next(cx));

        match next {
            Some(_) => {
                *this.count += 1;
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            None => Poll::Ready(*this.count),
        }
    }
}
