//! 在 `async-vfs` 模块中，我们提供了一个异步的虚拟文件系统框架，
//! 用于支持异步文件系统的实现。
//!
//! 在 basic.rs 文件中定义了基础的 vfs 接口：
//!     1. VfsNodeOps trait：定义了文件系统中的单个文件（包括普通文件和目录文件）的接口
//!         1. open
//!         2. release
//!         3. get_attr
//!         4. read_at
//!         5. write_at
//!         6. fsync
//!         7. truncate
//!         8. parent
//!         9. lookup
//!         10. create
//!         11. remove
//!         12. read_dir
//!         13. rename
//!         14. as_any
//!     2. VfsOps trait：定义了文件系统的接口
//!         1. mount
//!         2. format
//!         3. statfs
//!         4. root_dir
//!
//! vfs 目录下，提供了异步文件系统的实现，这些接口的返回结果都是 Future 对象（见目录结构）
//!
//! vfs_node 目录下，提供了异步文件系统节点的实现，这些接口的返回结果都是 Future 对象（见目录结构）
//!
//! path.rs 中提供了路径解析函数
//!
//! structs.rs 中定义了 VfsDirEntry、VfsNodeAttr、VfsNodePerm、VfsNodeType、FileSystemInfo 等结构体
//!
//! macros.rs 中定义了一些宏，给普通文件提供与目录操作相关的接口的虚拟实现，给目录文件提供与普通文件相关的接口的虚拟实现
#![cfg_attr(not(test), no_std)]
#![cfg_attr(test, feature(noop_waker))]

extern crate alloc;

mod macros;
pub mod path;
mod structs;

pub use crate::structs::{FileSystemInfo, VfsDirEntry, VfsNodeAttr, VfsNodePerm, VfsNodeType};

use alloc::sync::Arc;
use axerrno::{ax_err, AxError, AxResult};
use core::pin::Pin;
use core::task::{Context, Poll};

/// A wrapper of [`Arc<dyn VfsNodeOps>`].
pub type VfsNodeRef = Arc<dyn VfsNodeOps + Unpin + Send + Sync>;

/// Alias of [`AxError`].
pub type VfsError = AxError;

/// Alias of [`AxResult`].
pub type VfsResult<T = ()> = AxResult<T>;

use async_utils::async_trait;

/// Filesystem operations.
#[async_trait]
pub trait VfsOps: Send + Sync {
    /// Do something when the filesystem is mounted.
    fn poll_mount(
        self: Pin<&Self>,
        _cx: &mut Context<'_>,
        _path: &str,
        _mount_point: &VfsNodeRef,
    ) -> Poll<VfsResult> {
        Poll::Ready(Ok(()))
    }

    /// Do something when the filesystem is unmounted.
    fn umount(&self) -> VfsResult {
        Ok(())
    }

    /// Format the filesystem.
    fn poll_format(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<VfsResult> {
        Poll::Ready(ax_err!(Unsupported))
    }

    /// Get the attributes of the filesystem.
    fn poll_statfs(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<VfsResult<FileSystemInfo>> {
        Poll::Ready(ax_err!(Unsupported))
    }

    /// Get the root directory of the filesystem.
    fn root_dir(&self) -> VfsNodeRef {
        unimplemented!()
    }
}

/// Node (file/directory) operations.
#[async_trait]
pub trait VfsNodeOps: Send + Sync + Unpin {
    /// Do something when the node is opened.
    fn poll_open(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<VfsResult> {
        Poll::Ready(Ok(()))
    }

    /// Do something when the node is closed.
    fn release(&self) -> VfsResult {
        Ok(())
    }

    /// Get the attributes of the node.
    fn poll_get_attr(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<VfsResult<VfsNodeAttr>> {
        Poll::Ready(ax_err!(Unsupported))
    }

    // file operations:

    /// Read data from the file at the given offset.
    fn poll_read_at(
        self: Pin<&Self>,
        _cx: &mut Context<'_>,
        _offset: u64,
        _buf: &mut [u8],
    ) -> Poll<VfsResult<usize>> {
        Poll::Ready(ax_err!(InvalidInput))
    }

    /// Write data to the file at the given offset.
    fn poll_write_at(
        self: Pin<&Self>,
        _cx: &mut Context<'_>,
        _offset: u64,
        _buf: &[u8],
    ) -> Poll<VfsResult<usize>> {
        Poll::Ready(ax_err!(InvalidInput))
    }

    /// Flush the file, synchronize the data to disk.
    fn poll_fsync(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<VfsResult> {
        Poll::Ready(ax_err!(InvalidInput))
    }

    /// Truncate the file to the given size.
    fn poll_truncate(self: Pin<&Self>, _cx: &mut Context<'_>, _size: u64) -> Poll<VfsResult> {
        Poll::Ready(ax_err!(InvalidInput))
    }

    // directory operations:

    /// Get the parent directory of this directory.
    ///
    /// Return `None` if the node is a file.
    fn parent(&self) -> Option<VfsNodeRef> {
        None
    }

    /// Lookup the node with given `path` in the directory.
    ///
    /// Return the node if found.
    fn lookup(self: Arc<Self>, _path: &str) -> VfsResult<VfsNodeRef> {
        ax_err!(Unsupported)
    }

    /// Create a new node with the given `path` in the directory
    ///
    /// Return [`Ok(())`](Ok) if it already exists.
    fn poll_create(
        self: Pin<&Self>,
        _cx: &mut Context<'_>,
        _path: &str,
        _ty: VfsNodeType,
    ) -> Poll<VfsResult> {
        Poll::Ready(ax_err!(Unsupported))
    }

    /// Remove the node with the given `path` in the directory.
    fn poll_remove(self: Pin<&Self>, _cx: &mut Context<'_>, _path: &str) -> Poll<VfsResult> {
        Poll::Ready(ax_err!(Unsupported))
    }

    /// Read directory entries into `dirents`, starting from `start_idx`.
    fn poll_read_dir(
        self: Pin<&Self>,
        _cx: &mut Context<'_>,
        _start_idx: usize,
        _dirents: &mut [VfsDirEntry],
    ) -> Poll<VfsResult<usize>> {
        Poll::Ready(ax_err!(Unsupported))
    }

    /// Renames or moves existing file or directory.
    fn poll_rename(
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

#[doc(hidden)]
pub mod __priv {
    pub use alloc::sync::Arc;
    pub use axerrno::ax_err;
}
