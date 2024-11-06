use alloc::vec::Vec;
use async_vfs::{impl_vfs_non_dir_default, VfsNodeAttr, VfsNodeOps, VfsResult};
use core::{
    pin::Pin,
    task::{Context, Poll},
};
use spin::RwLock;

/// The file node in the RAM filesystem.
///
/// It implements [`axfs_vfs::VfsNodeOps`].
pub struct FileNode {
    content: RwLock<Vec<u8>>,
}

impl FileNode {
    /// To get the environment variables of the application
    pub const fn new() -> Self {
        Self {
            content: RwLock::new(Vec::new()),
        }
    }
}

impl VfsNodeOps for FileNode {
    fn poll_get_attr(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<VfsResult<VfsNodeAttr>> {
        Poll::Ready(Ok(VfsNodeAttr::new_file(self.content.read().len() as _, 0)))
    }

    fn poll_truncate(self: Pin<&Self>, _cx: &mut Context<'_>, size: u64) -> Poll<VfsResult> {
        let mut content = self.content.write();
        if size < content.len() as u64 {
            content.truncate(size as _);
        } else {
            content.resize(size as _, 0);
        }
        Poll::Ready(Ok(()))
    }

    fn poll_read_at(
        self: Pin<&Self>,
        _cx: &mut Context<'_>,
        offset: u64,
        buf: &mut [u8],
    ) -> Poll<VfsResult<usize>> {
        let content = self.content.read();
        let start = content.len().min(offset as usize);
        let end = content.len().min(offset as usize + buf.len());
        let src = &content[start..end];
        buf[..src.len()].copy_from_slice(src);
        Poll::Ready(Ok(src.len()))
    }

    fn poll_write_at(
        self: Pin<&Self>,
        _cx: &mut Context<'_>,
        offset: u64,
        buf: &[u8],
    ) -> Poll<VfsResult<usize>> {
        // fn write_at(&self, offset: u64, buf: &[u8]) -> VfsResult<usize> {
        let offset = offset as usize;
        let mut content = self.content.write();
        if offset + buf.len() > content.len() {
            content.resize(offset + buf.len(), 0);
        }
        let dst = &mut content[offset..offset + buf.len()];
        dst.copy_from_slice(&buf[..dst.len()]);
        Poll::Ready(Ok(buf.len()))
    }

    impl_vfs_non_dir_default! {}
}
