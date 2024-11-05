use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use crate::{VfsNodeRef, VfsOps};

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct RootDirFuture<'a, T: Unpin + ?Sized> {
    pub(crate) fs: &'a T,
}

impl<T: VfsOps + Unpin + ?Sized> Future for RootDirFuture<'_, T> {
    type Output = VfsNodeRef;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { fs } = self.get_mut();
        Pin::new(*fs).root_dir(cx)
    }
}
