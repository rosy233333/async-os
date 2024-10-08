use core::cmp::Ordering;
use core::future::Future;
use core::pin::Pin;

use pin_project_lite::pin_project;

use super::partial_cmp::PartialCmpFuture;
use super::{AsyncStream, Stream};
use core::task::{Context, Poll};

pin_project! {
    /// Determines if the elements of this `Stream` are lexicographically
    /// less or equal to those of another.
    #[doc(hidden)]
    pub struct LeFuture<L: AsyncStream, R: AsyncStream> {
        #[pin]
        partial_cmp: PartialCmpFuture<L, R>,
    }
}

impl<L: AsyncStream, R: AsyncStream> LeFuture<L, R>
where
    L::Item: PartialOrd<R::Item>,
{
    pub(super) fn new(l: L, r: R) -> Self {
        Self {
            partial_cmp: l.partial_cmp(r),
        }
    }
}

impl<L: AsyncStream, R: AsyncStream> Future for LeFuture<L, R>
where
    L: AsyncStream + Sized,
    R: AsyncStream + Sized,
    L::Item: PartialOrd<R::Item>,
{
    type Output = bool;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let result = futures_core::ready!(self.project().partial_cmp.poll(cx));

        match result {
            Some(Ordering::Less) | Some(Ordering::Equal) => Poll::Ready(true),
            _ => Poll::Ready(false),
        }
    }
}