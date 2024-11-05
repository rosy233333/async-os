use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use crate::{VfsNodeOps, VfsResult};

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct WriteAtFuture<'a, T: Unpin + ?Sized> {
    pub(crate) vnode: &'a T,
    pub(crate) offset: u64,
    pub(crate) buf: &'a [u8],
}

impl<T: VfsNodeOps + Unpin + ?Sized> Future for WriteAtFuture<'_, T> {
    type Output = VfsResult<usize>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { vnode, offset, buf } = self.get_mut();
        Pin::new(*vnode).write_at(cx, *offset, buf)
    }
}
