use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use crate::{VfsOps, VfsResult};

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct FormatFuture<'a, T: Unpin + ?Sized> {
    pub(crate) fs: &'a T,
}

impl<T: VfsOps + Unpin + ?Sized> Future for FormatFuture<'_, T> {
    type Output = VfsResult;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { fs } = self.get_mut();
        Pin::new(*fs).format(cx)
    }
}
