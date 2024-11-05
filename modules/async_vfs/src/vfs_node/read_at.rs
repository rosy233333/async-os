use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use crate::{VfsNodeOps, VfsResult};

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct ReadAtFuture<'a, T: Unpin + ?Sized> {
    pub(crate) vnode: &'a T,
    pub(crate) offset: u64,
    pub(crate) buf: &'a mut [u8],
}

impl<T: VfsNodeOps + Unpin + ?Sized> Future for ReadAtFuture<'_, T> {
    type Output = VfsResult<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { vnode, offset, buf } = self.get_mut();
        Pin::new(*vnode).read_at(cx, *offset, buf)
    }
}
