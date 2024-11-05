use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use crate::{VfsNodeRef, VfsOps, VfsResult};

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct MountFuture<'a, T: Unpin + ?Sized> {
    pub(crate) fs: &'a T,
    pub(crate) path: &'a str,
    pub(crate) mount_point: VfsNodeRef,
}

impl<T: VfsOps + Unpin + ?Sized> Future for MountFuture<'_, T> {
    type Output = VfsResult;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self {
            fs,
            path,
            mount_point,
        } = self.get_mut();
        Pin::new(*fs).mount(cx, path, mount_point.clone())
    }
}
