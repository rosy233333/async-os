use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use crate::{VfsNodeOps, VfsNodeRef, VfsResult};

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct LookupFuture<'a, T: Unpin + ?Sized> {
    pub(crate) vnode: &'a T,
    pub(crate) path: &'a str,
}

impl<T: VfsNodeOps + Unpin + ?Sized> Future for LookupFuture<'_, T> {
    type Output = VfsResult<VfsNodeRef>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { vnode, path } = self.get_mut();
        Pin::new(*vnode).lookup(cx, path)
    }
}
