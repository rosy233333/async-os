use core::default::Default;
use core::future::Future;
use core::pin::Pin;
use pin_project_lite::pin_project;

use super::AsyncStream;
use core::task::{Context, Poll};

pin_project! {
    #[derive(Debug)]
    pub struct PartitionFuture<S, F, B> {
        #[pin]
        stream: S,
        f: F,
        res: Option<(B, B)>,
    }
}

impl<S, F, B: Default> PartitionFuture<S, F, B> {
    pub(super) fn new(stream: S, f: F) -> Self {
        Self {
            stream,
            f,
            res: Some((B::default(), B::default())),
        }
    }
}

impl<S, F, B> Future for PartitionFuture<S, F, B>
where
    S: AsyncStream + Sized,
    F: FnMut(&S::Item) -> bool,
    B: Default + Extend<S::Item>,
{
    type Output = (B, B);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();

        loop {
            let next = futures_core::ready!(this.stream.as_mut().poll_next(cx));

            match next {
                Some(v) => {
                    let res = this.res.as_mut().unwrap();

                    if (this.f)(&v) {
                        res.0.extend(Some(v))
                    } else {
                        res.1.extend(Some(v))
                    }
                }
                None => return Poll::Ready(this.res.take().unwrap()),
            }
        }
    }
}
