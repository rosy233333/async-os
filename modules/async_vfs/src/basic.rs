//! 这里定义了基本的 Vfs 相关的操作
//! 在其他的使用 vfs 的模块中需要实现这里的 VfsOps、VfsNodeOps trait
//!
use alloc::boxed::Box;
use alloc::sync::Arc;
use axerrno::{ax_err, AxError, AxResult};
use core::ops::{Deref, DerefMut};
use core::pin::Pin;
use core::task::{Context, Poll};

use crate::structs::{FileSystemInfo, VfsDirEntry, VfsNodeAttr, VfsNodeType};

/// A wrapper of [`Arc<dyn VfsNodeOps>`].
pub type VfsNodeRef = Arc<dyn VfsNodeOps + Unpin>;

/// Alias of [`AxError`].
pub type VfsError = AxError;

/// Alias of [`AxResult`].
pub type VfsResult<T = ()> = AxResult<T>;

/// Filesystem operations.
pub trait VfsOps: Send + Sync {
    /// Do something when the filesystem is mounted.
    fn mount(
        self: Pin<&Self>,
        _cx: &mut Context<'_>,
        _path: &str,
        _mount_point: VfsNodeRef,
    ) -> Poll<VfsResult> {
        Poll::Ready(Ok(()))
    }

    /// Do something when the filesystem is unmounted.
    fn umount(&self) -> VfsResult {
        Ok(())
    }

    /// Format the filesystem.
    fn format(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<VfsResult> {
        Poll::Ready(ax_err!(Unsupported))
    }

    /// Get the attributes of the filesystem.
    fn statfs(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<VfsResult<FileSystemInfo>> {
        Poll::Ready(ax_err!(Unsupported))
    }

    /// Get the root directory of the filesystem.
    fn root_dir(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<VfsNodeRef>;
}

macro_rules! deref_async_vfsops {
    () => {
        fn mount(
            self: Pin<&Self>,
            cx: &mut Context<'_>,
            path: &str,
            mount_point: VfsNodeRef,
        ) -> Poll<VfsResult> {
            Pin::new(&**self).mount(cx, path, mount_point)
        }

        /// Format the filesystem.
        fn format(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<VfsResult> {
            Pin::new(&**self).format(cx)
        }

        /// Get the attributes of the filesystem.
        fn statfs(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<VfsResult<FileSystemInfo>> {
            Pin::new(&**self).statfs(cx)
        }

        /// Get the root directory of the filesystem.
        fn root_dir(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<VfsNodeRef> {
            Pin::new(&**self).root_dir(cx)
        }
    };
}

impl<T: ?Sized + VfsOps + Unpin> VfsOps for Box<T> {
    deref_async_vfsops!();
}

impl<T: ?Sized + VfsOps + Unpin> VfsOps for &mut T {
    deref_async_vfsops!();
}

impl<T: ?Sized + VfsOps + Unpin> VfsOps for Arc<T> {
    deref_async_vfsops!();
}

impl<P> VfsOps for Pin<P>
where
    P: DerefMut + Unpin + Send + Sync,
    P::Target: VfsOps,
{
    fn mount(
        self: Pin<&Self>,
        cx: &mut Context<'_>,
        path: &str,
        mount_point: VfsNodeRef,
    ) -> Poll<VfsResult> {
        self.get_ref().as_ref().mount(cx, path, mount_point)
    }

    fn format(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<VfsResult> {
        self.get_ref().as_ref().format(cx)
    }

    fn statfs(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<VfsResult<FileSystemInfo>> {
        self.get_ref().as_ref().statfs(cx)
    }

    fn root_dir(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<VfsNodeRef> {
        self.get_ref().as_ref().root_dir(cx)
    }
}

/// Node (file/directory) operations.
pub trait VfsNodeOps: Send + Sync {
    /// Do something when the node is opened.
    fn open(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<VfsResult> {
        Poll::Ready(Ok(()))
    }

    /// Do something when the node is closed.
    fn release(&self) -> VfsResult {
        Ok(())
    }

    /// Get the attributes of the node.
    fn get_attr(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<VfsResult<VfsNodeAttr>> {
        Poll::Ready(ax_err!(Unsupported))
    }

    // file operations:

    /// Read data from the file at the given offset.
    fn read_at(
        self: Pin<&Self>,
        _cx: &mut Context<'_>,
        _offset: u64,
        _buf: &mut [u8],
    ) -> Poll<VfsResult<usize>> {
        Poll::Ready(ax_err!(InvalidInput))
    }

    /// Write data to the file at the given offset.
    fn write_at(
        self: Pin<&Self>,
        _cx: &mut Context<'_>,
        _offset: u64,
        _buf: &[u8],
    ) -> Poll<VfsResult<usize>> {
        Poll::Ready(ax_err!(InvalidInput))
    }

    /// Flush the file, synchronize the data to disk.
    fn fsync(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<VfsResult> {
        Poll::Ready(ax_err!(InvalidInput))
    }

    /// Truncate the file to the given size.
    fn truncate(self: Pin<&Self>, _cx: &mut Context<'_>, _size: u64) -> Poll<VfsResult> {
        Poll::Ready(ax_err!(InvalidInput))
    }

    // directory operations:

    /// Get the parent directory of this directory.
    ///
    /// Return `None` if the node is a file.
    fn parent(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<Option<VfsNodeRef>> {
        Poll::Ready(None)
    }

    /// Lookup the node with given `path` in the directory.
    ///
    /// Return the node if found.
    fn lookup(self: Pin<&Self>, _cx: &mut Context<'_>, _path: &str) -> Poll<VfsResult<VfsNodeRef>> {
        Poll::Ready(ax_err!(Unsupported))
    }

    /// Create a new node with the given `path` in the directory
    ///
    /// Return [`Ok(())`](Ok) if it already exists.
    fn create(
        self: Pin<&Self>,
        _cx: &mut Context<'_>,
        _path: &str,
        _ty: VfsNodeType,
    ) -> Poll<VfsResult> {
        Poll::Ready(ax_err!(Unsupported))
    }

    /// Remove the node with the given `path` in the directory.
    fn remove(self: Pin<&Self>, _cx: &mut Context<'_>, _path: &str) -> Poll<VfsResult> {
        Poll::Ready(ax_err!(Unsupported))
    }

    /// Read directory entries into `dirents`, starting from `start_idx`.
    fn read_dir(
        self: Pin<&Self>,
        _cx: &mut Context<'_>,
        _start_idx: usize,
        _dirents: &mut [VfsDirEntry],
    ) -> Poll<VfsResult<usize>> {
        Poll::Ready(ax_err!(Unsupported))
    }

    /// Renames or moves existing file or directory.
    fn rename(
        self: Pin<&Self>,
        _cx: &mut Context<'_>,
        _src_path: &str,
        _dst_path: &str,
    ) -> Poll<VfsResult> {
        Poll::Ready(ax_err!(Unsupported))
    }

    /// Convert `&self` to [`&dyn Any`][1] that can use
    /// [`Any::downcast_ref`][2].
    ///
    /// [1]: core::any::Any
    /// [2]: core::any::Any#method.downcast_ref
    fn as_any(&self) -> &dyn core::any::Any {
        unimplemented!()
    }
}

macro_rules! deref_async_vfsnodeops {
    () => {
        fn open(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<VfsResult> {
            Pin::new(&**self).open(cx)
        }

        fn get_attr(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<VfsResult<VfsNodeAttr>> {
            Pin::new(&**self).get_attr(cx)
        }

        fn read_at(
            self: Pin<&Self>,
            cx: &mut Context<'_>,
            offset: u64,
            buf: &mut [u8],
        ) -> Poll<VfsResult<usize>> {
            Pin::new(&**self).read_at(cx, offset, buf)
        }

        fn write_at(
            self: Pin<&Self>,
            cx: &mut Context<'_>,
            offset: u64,
            buf: &[u8],
        ) -> Poll<VfsResult<usize>> {
            Pin::new(&**self).write_at(cx, offset, buf)
        }

        fn fsync(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<VfsResult> {
            Pin::new(&**self).fsync(cx)
        }

        fn truncate(self: Pin<&Self>, cx: &mut Context<'_>, size: u64) -> Poll<VfsResult> {
            Pin::new(&**self).truncate(cx, size)
        }

        fn parent(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<Option<VfsNodeRef>> {
            Pin::new(&**self).parent(cx)
        }

        fn lookup(
            self: Pin<&Self>,
            cx: &mut Context<'_>,
            path: &str,
        ) -> Poll<VfsResult<VfsNodeRef>> {
            Pin::new(&**self).lookup(cx, path)
        }

        fn create(
            self: Pin<&Self>,
            cx: &mut Context<'_>,
            path: &str,
            ty: VfsNodeType,
        ) -> Poll<VfsResult> {
            Pin::new(&**self).create(cx, path, ty)
        }

        fn remove(self: Pin<&Self>, cx: &mut Context<'_>, path: &str) -> Poll<VfsResult> {
            Pin::new(&**self).remove(cx, path)
        }

        fn read_dir(
            self: Pin<&Self>,
            cx: &mut Context<'_>,
            start_idx: usize,
            dirents: &mut [VfsDirEntry],
        ) -> Poll<VfsResult<usize>> {
            Pin::new(&**self).read_dir(cx, start_idx, dirents)
        }

        fn rename(
            self: Pin<&Self>,
            cx: &mut Context<'_>,
            src_path: &str,
            dst_path: &str,
        ) -> Poll<VfsResult> {
            Pin::new(&**self).rename(cx, src_path, dst_path)
        }
    };
}

impl<T: ?Sized + VfsNodeOps + Unpin> VfsNodeOps for Box<T> {
    deref_async_vfsnodeops!();
}

impl<T: ?Sized + VfsNodeOps + Unpin> VfsNodeOps for &mut T {
    deref_async_vfsnodeops!();
}

impl<T: ?Sized + VfsNodeOps + Unpin> VfsNodeOps for Arc<T> {
    deref_async_vfsnodeops!();
}

impl<P> VfsNodeOps for Pin<P>
where
    P: Deref + Unpin + Send + Sync,
    P::Target: VfsNodeOps,
{
    fn open(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<VfsResult> {
        self.get_ref().as_ref().open(cx)
    }

    fn get_attr(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<VfsResult<VfsNodeAttr>> {
        self.get_ref().as_ref().get_attr(cx)
    }

    fn read_at(
        self: Pin<&Self>,
        cx: &mut Context<'_>,
        offset: u64,
        buf: &mut [u8],
    ) -> Poll<VfsResult<usize>> {
        self.get_ref().as_ref().read_at(cx, offset, buf)
    }

    fn write_at(
        self: Pin<&Self>,
        cx: &mut Context<'_>,
        offset: u64,
        buf: &[u8],
    ) -> Poll<VfsResult<usize>> {
        self.get_ref().as_ref().write_at(cx, offset, buf)
    }

    fn fsync(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<VfsResult> {
        self.get_ref().as_ref().fsync(cx)
    }

    fn truncate(self: Pin<&Self>, cx: &mut Context<'_>, size: u64) -> Poll<VfsResult> {
        self.get_ref().as_ref().truncate(cx, size)
    }

    fn parent(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<Option<VfsNodeRef>> {
        self.get_ref().as_ref().parent(cx)
    }

    fn lookup(self: Pin<&Self>, cx: &mut Context<'_>, path: &str) -> Poll<VfsResult<VfsNodeRef>> {
        self.get_ref().as_ref().lookup(cx, path)
    }

    fn create(
        self: Pin<&Self>,
        cx: &mut Context<'_>,
        path: &str,
        ty: VfsNodeType,
    ) -> Poll<VfsResult> {
        self.get_ref().as_ref().create(cx, path, ty)
    }

    fn remove(self: Pin<&Self>, cx: &mut Context<'_>, path: &str) -> Poll<VfsResult> {
        self.get_ref().as_ref().remove(cx, path)
    }

    fn read_dir(
        self: Pin<&Self>,
        cx: &mut Context<'_>,
        start_idx: usize,
        dirents: &mut [VfsDirEntry],
    ) -> Poll<VfsResult<usize>> {
        self.get_ref().as_ref().read_dir(cx, start_idx, dirents)
    }

    fn rename(
        self: Pin<&Self>,
        cx: &mut Context<'_>,
        src_path: &str,
        dst_path: &str,
    ) -> Poll<VfsResult> {
        self.get_ref().as_ref().rename(cx, src_path, dst_path)
    }
}
