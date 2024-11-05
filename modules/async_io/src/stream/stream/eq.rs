use core::future::Future;
use core::pin::Pin;

use pin_project_lite::pin_project;

use super::fuse::Fuse;
use super::{AsyncStream, Stream};
use core::task::{Context, Poll};

pin_project! {
    // Lexicographically compares the elements of this `Stream` with those
    // of another.
    #[doc(hidden)]
    pub struct EqFuture<L: Stream, R: Stream> {
        #[pin]
        l: Fuse<L>,
        #[pin]
        r: Fuse<R>,
    }
}

impl<L: AsyncStream, R: AsyncStream> EqFuture<L, R>
where
    L::Item: PartialEq<R::Item>,
{
    pub(super) fn new(l: L, r: R) -> Self {
        Self {
            l: l.fuse(),
            r: r.fuse(),
        }
    }
}

impl<L: AsyncStream, R: AsyncStream> Future for EqFuture<L, R>
where
    L: AsyncStream + Sized,
    R: AsyncStream + Sized,
    L::Item: PartialEq<R::Item>,
{
    type Output = bool;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        loop {
            let l_val = core::task::ready!(this.l.as_mut().poll_next(cx));
            let r_val = core::task::ready!(this.r.as_mut().poll_next(cx));

            if this.l.done && this.r.done {
                return Poll::Ready(true);
            }

            match (l_val, r_val) {
                (Some(l), Some(r)) if l != r => {
                    return Poll::Ready(false);
                }
                _ => {}
            }
        }
    }
}
