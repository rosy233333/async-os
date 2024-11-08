use core::cmp::Ordering;
use core::future::Future;
use core::pin::Pin;

use pin_project_lite::pin_project;

use super::AsyncStream;
use core::task::{Context, Poll};

pin_project! {
    #[doc(hidden)]
    #[allow(missing_debug_implementations)]
    pub struct MinByFuture<S, F, T> {
        #[pin]
        stream: S,
        compare: F,
        min: Option<T>,
    }
}

impl<S, F, T> MinByFuture<S, F, T> {
    pub(super) fn new(stream: S, compare: F) -> Self {
        Self {
            stream,
            compare,
            min: None,
        }
    }
}

impl<S, F> Future for MinByFuture<S, F, S::Item>
where
    S: AsyncStream + Unpin + Sized,
    S::Item: Copy,
    F: FnMut(&S::Item, &S::Item) -> Ordering,
{
    type Output = Option<S::Item>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let next = core::task::ready!(this.stream.poll_next(cx));

        match next {
            Some(new) => {
                match this.min.take() {
                    None => *this.min = Some(new),
                    Some(old) => match (this.compare)(&new, &old) {
                        Ordering::Less => *this.min = Some(new),
                        _ => *this.min = Some(old),
                    },
                }
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            None => Poll::Ready(*this.min),
        }
    }
}
