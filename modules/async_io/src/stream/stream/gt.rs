use core::cmp::Ordering;
use core::future::Future;
use core::pin::Pin;

use pin_project_lite::pin_project;

use super::partial_cmp::PartialCmpFuture;
use super::{AsyncStream, Stream};
use core::task::{Context, Poll};

pin_project! {
    // Determines if the elements of this `Stream` are lexicographically
    // greater than those of another.
    #[doc(hidden)]
    #[allow(missing_debug_implementations)]
    pub struct GtFuture<L: AsyncStream, R: AsyncStream> {
        #[pin]
        partial_cmp: PartialCmpFuture<L, R>,
    }
}

impl<L: AsyncStream, R: AsyncStream> GtFuture<L, R>
where
    L::Item: PartialOrd<R::Item>,
{
    pub(super) fn new(l: L, r: R) -> Self {
        Self {
            partial_cmp: l.partial_cmp(r),
        }
    }
}

impl<L: AsyncStream, R: AsyncStream> Future for GtFuture<L, R>
where
    L: AsyncStream + Sized,
    R: AsyncStream + Sized,
    L::Item: PartialOrd<R::Item>,
{
    type Output = bool;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let result = core::task::ready!(self.project().partial_cmp.poll(cx));

        match result {
            Some(Ordering::Greater) => Poll::Ready(true),
            _ => Poll::Ready(false),
        }
    }
}
