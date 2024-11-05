use core::future::Future;
use core::pin::Pin;

use crate::{self as io, AsyncRead};
use core::task::{Context, Poll};

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct ReadFuture<'a, T: Unpin + ?Sized> {
    pub(crate) reader: &'a mut T,
    pub(crate) buf: &'a mut [u8],
}

impl<T: AsyncRead + Unpin + ?Sized> Future for ReadFuture<'_, T> {
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { reader, buf } = &mut *self;
        Pin::new(reader).read(cx, buf)
    }
}
