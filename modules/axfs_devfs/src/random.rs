use core::ops::DerefMut;
use core::{
    pin::Pin,
    task::{Context, Poll},
};

use async_vfs::{VfsNodeAttr, VfsNodeOps, VfsNodePerm, VfsNodeType, VfsResult};
use rand::{rngs::SmallRng, Fill, SeedableRng};
use spin::Mutex;

/// A random device behaves like `/dev/random` or `/dev/urandom`.
///
/// It always returns a chunk of random bytes when read, and all writes are discarded.
///
/// TODO: update entropy pool with data written.
pub struct RandomDev(Mutex<SmallRng>);

impl Default for RandomDev {
    fn default() -> Self {
        Self(Mutex::new(SmallRng::from_seed([0; 32])))
    }
}

impl VfsNodeOps for RandomDev {
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
        buf: &mut [u8],
    ) -> Poll<VfsResult<usize>> {
        buf.try_fill(self.0.lock().deref_mut()).unwrap();
        Poll::Ready(Ok(buf.len()))
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
