use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};

use crate::{FileSystemInfo, VfsOps, VfsResult};

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct StatFsFuture<'a, T: Unpin + ?Sized> {
    pub(crate) fs: &'a T,
}

impl<T: VfsOps + Unpin + ?Sized> Future for StatFsFuture<'_, T> {
    type Output = VfsResult<FileSystemInfo>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { fs } = self.get_mut();
        Pin::new(*fs).statfs(cx)
    }
}
