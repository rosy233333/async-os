use async_vfs::{VfsNodeAttr, VfsNodeOps, VfsNodePerm, VfsNodeType, VfsResult};
use core::{
    pin::Pin,
    task::{Context, Poll},
};

/// A null device behaves like `/dev/null`.
///
/// Nothing can be read and all writes are discarded.
pub struct NullDev;

impl VfsNodeOps for NullDev {
    fn poll_get_attr(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<VfsResult<VfsNodeAttr>> {
        Poll::Ready(Ok(VfsNodeAttr::new(
            VfsNodePerm::default_file(),
            VfsNodeType::CharDevice,
            0,
            0,
        )))
    }

    fn poll_read_at(
        self: Pin<&Self>,
        _cx: &mut Context<'_>,
        _offset: u64,
        _buf: &mut [u8],
    ) -> Poll<VfsResult<usize>> {
        Poll::Ready(Ok(0))
    }

    fn poll_write_at(
        self: Pin<&Self>,
        _cx: &mut Context<'_>,
        _offset: u64,
        buf: &[u8],
    ) -> Poll<VfsResult<usize>> {
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_truncate(self: Pin<&Self>, _cx: &mut Context<'_>, _size: u64) -> Poll<VfsResult> {
        Poll::Ready(Ok(()))
    }

    async_vfs::impl_vfs_non_dir_default! {}
}
