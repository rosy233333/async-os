use core::future::Future;
use core::pin::Pin;

use crate::{self as io, AsyncSeek, SeekFrom};
use core::task::{Context, Poll};

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct SeekFuture<'a, T: Unpin + ?Sized> {
    pub(crate) seeker: &'a mut T,
    pub(crate) pos: SeekFrom,
}

impl<T: AsyncSeek + Unpin + ?Sized> Future for SeekFuture<'_, T> {
    type Output = io::Result<u64>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let pos = self.pos;
        Pin::new(&mut *self.seeker).seek(cx, pos)
    }
}
