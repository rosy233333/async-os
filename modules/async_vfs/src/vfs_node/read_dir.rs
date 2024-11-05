use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use crate::{VfsDirEntry, VfsNodeOps, VfsResult};

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct ReadDirFuture<'a, T: Unpin + ?Sized> {
    pub(crate) vnode: &'a T,
    pub(crate) start_idx: usize,
    pub(crate) dirents: &'a mut [VfsDirEntry],
}

impl<T: VfsNodeOps + Unpin + ?Sized> Future for ReadDirFuture<'_, T> {
    type Output = VfsResult<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self {
            vnode,
            start_idx,
            dirents,
        } = self.get_mut();
        Pin::new(*vnode).read_dir(cx, *start_idx, dirents)
    }
}
