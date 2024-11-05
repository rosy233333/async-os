use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use crate::{VfsNodeOps, VfsResult};

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct RenameFuture<'a, T: Unpin + ?Sized> {
    pub(crate) vnode: &'a T,
    pub(crate) src_path: &'a str,
    pub(crate) dst_path: &'a str,
}

impl<T: VfsNodeOps + Unpin + ?Sized> Future for RenameFuture<'_, T> {
    type Output = VfsResult;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self {
            vnode,
            src_path,
            dst_path,
        } = self.get_mut();
        Pin::new(*vnode).rename(cx, src_path, dst_path)
    }
}
