use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use crate::{VfsNodeOps, VfsNodeType, VfsResult};

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct CreateFuture<'a, T: Unpin + ?Sized> {
    pub(crate) vnode: &'a T,
    pub(crate) path: &'a str,
    pub(crate) ty: VfsNodeType,
}

impl<T: VfsNodeOps + Unpin + ?Sized> Future for CreateFuture<'_, T> {
    type Output = VfsResult;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { vnode, path, ty } = self.get_mut();
        Pin::new(*vnode).create(cx, path, *ty)
    }
}
