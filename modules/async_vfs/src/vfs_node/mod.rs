use create::CreateFuture;
use fsync::FsyncFuture;
use get_attr::GetAttrFuture;
use lookup::LookupFuture;
use open::OpenFuture;
use parent::ParentFuture;
use read_at::ReadAtFuture;
use read_dir::ReadDirFuture;
use remove::RemoveFuture;
use rename::RenameFuture;
use truncate::TruncateFuture;
use write_at::WriteAtFuture;

use crate::{VfsDirEntry, VfsNodeOps, VfsNodeType};

mod create;
mod fsync;
mod get_attr;
mod lookup;
mod open;
mod parent;
mod read_at;
mod read_dir;
mod remove;
mod rename;
mod truncate;
mod write_at;

pub trait AsyncVfsNodeOps: VfsNodeOps {
    /// Do something when the node is opened.
    fn open<'a>(self: &'a Self) -> OpenFuture<'a, Self>
    where
        Self: Unpin,
    {
        OpenFuture { vnode: self }
    }

    /// Get the attributes of the node.
    fn get_attr<'a>(self: &'a Self) -> GetAttrFuture<'a, Self>
    where
        Self: Unpin,
    {
        GetAttrFuture { vnode: self }
    }

    // file operations:

    /// Read data from the file at the given offset.
    fn read_at<'a>(self: &'a Self, offset: u64, buf: &'a mut [u8]) -> ReadAtFuture<'a, Self>
    where
        Self: Unpin,
    {
        ReadAtFuture {
            vnode: self,
            offset,
            buf,
        }
    }

    /// Write data to the file at the given offset.
    fn write_at<'a>(self: &'a Self, offset: u64, buf: &'a [u8]) -> WriteAtFuture<'a, Self>
    where
        Self: Unpin,
    {
        WriteAtFuture {
            vnode: self,
            offset,
            buf,
        }
    }

    /// Flush the file, synchronize the data to disk.
    fn fsync<'a>(self: &'a Self) -> FsyncFuture<'a, Self>
    where
        Self: Unpin,
    {
        FsyncFuture { vnode: self }
    }

    /// Truncate the file to the given size.
    fn truncate<'a>(self: &'a Self, size: u64) -> TruncateFuture<'a, Self>
    where
        Self: Unpin,
    {
        TruncateFuture { vnode: self, size }
    }

    // directory operations:

    /// Get the parent directory of this directory.
    ///
    /// Return `None` if the node is a file.
    fn parent<'a>(self: &'a Self) -> ParentFuture<'a, Self>
    where
        Self: Unpin,
    {
        ParentFuture { vnode: self }
    }

    /// Lookup the node with given `path` in the directory.
    ///
    /// Return the node if found.
    fn lookup<'a>(self: &'a Self, path: &'a str) -> LookupFuture<'a, Self>
    where
        Self: Unpin,
    {
        LookupFuture { vnode: self, path }
    }

    /// Create a new node with the given `path` in the directory
    ///
    /// Return [`Ok(())`](Ok) if it already exists.
    fn create<'a>(self: &'a Self, path: &'a str, ty: VfsNodeType) -> CreateFuture<'a, Self>
    where
        Self: Unpin,
    {
        CreateFuture {
            vnode: self,
            path,
            ty,
        }
    }

    /// Remove the node with the given `path` in the directory.
    fn remove<'a>(self: &'a Self, path: &'a str) -> RemoveFuture<'a, Self>
    where
        Self: Unpin,
    {
        RemoveFuture { vnode: self, path }
    }

    /// Read directory entries into `dirents`, starting from `start_idx`.
    fn read_dir<'a>(
        self: &'a Self,
        start_idx: usize,
        dirents: &'a mut [VfsDirEntry],
    ) -> ReadDirFuture<'a, Self>
    where
        Self: Unpin,
    {
        ReadDirFuture {
            vnode: self,
            start_idx,
            dirents,
        }
    }

    /// Renames or moves existing file or directory.
    fn rename<'a>(self: &'a Self, src_path: &'a str, dst_path: &'a str) -> RenameFuture<'a, Self>
    where
        Self: Unpin,
    {
        RenameFuture {
            vnode: self,
            src_path,
            dst_path,
        }
    }
}

impl<T: VfsNodeOps + ?Sized> AsyncVfsNodeOps for T {}

#[cfg(test)]
mod test {
    use crate::AsyncVfsNodeOps;
    use crate::{VfsNodeOps, VfsNodeRef, VfsResult};
    use alloc::boxed::Box;
    use core::{
        future::Future,
        pin::Pin,
        task::{Context, Poll, Waker},
    };
    use std::{fs::*, path::Path};

    pub struct DirNode {
        inner: String,
    }

    impl DirNode {
        pub fn new(inner: String) -> Self {
            Self { inner }
        }
    }

    impl VfsNodeOps for DirNode {
        fn lookup(
            self: Pin<&Self>,
            _cx: &mut Context<'_>,
            _path: &str,
        ) -> Poll<VfsResult<VfsNodeRef>> {
            let dir = read_dir(Path::new(&self.inner)).unwrap();
            for dir_entry in dir {
                println!("{:?}", dir_entry);
            }
            Poll::Pending
        }
    }

    #[test]
    fn test_lookup() {
        let waker = Waker::noop();
        let mut cx = Context::from_waker(&waker);
        let dir_node = DirNode::new(String::from("./src"));
        let fut = async {
            let _ = dir_node.lookup("mod.rs").await;
        };
        let _ = Box::pin(fut).as_mut().poll(&mut cx);
    }
}
