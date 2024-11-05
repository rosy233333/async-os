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
mod structs;

mod basic;
pub mod path;
mod vfs;
mod vfs_node;

pub use crate::structs::{FileSystemInfo, VfsDirEntry, VfsNodeAttr, VfsNodePerm, VfsNodeType};
pub use basic::{VfsError, VfsNodeOps, VfsNodeRef, VfsOps, VfsResult};
pub use vfs::AsyncVfsOps;
pub use vfs_node::AsyncVfsNodeOps;

#[doc(hidden)]
pub mod __priv {
    pub use alloc::sync::Arc;
    pub use axerrno::ax_err;
}
