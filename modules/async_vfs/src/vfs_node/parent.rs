use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use crate::{VfsNodeOps, VfsNodeRef};

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct ParentFuture<'a, T: Unpin + ?Sized> {
    pub(crate) vnode: &'a T,
}

impl<T: VfsNodeOps + Unpin + ?Sized> Future for ParentFuture<'_, T> {
    type Output = Option<VfsNodeRef>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { vnode } = self.get_mut();
        Pin::new(*vnode).parent(cx)
    }
}
