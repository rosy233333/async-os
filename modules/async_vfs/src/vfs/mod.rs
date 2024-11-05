use crate::{VfsNodeRef, VfsOps};

mod format;
mod mount;
mod root_dir;
mod statfs;

use format::FormatFuture;
use mount::MountFuture;
use root_dir::RootDirFuture;
use statfs::StatFsFuture;

pub trait AsyncVfsOps: VfsOps {
    /// Do something when the filesystem is mounted.
    fn mount<'a>(&'a self, path: &'a str, mount_point: VfsNodeRef) -> MountFuture<'a, Self>
    where
        Self: Unpin,
    {
        MountFuture {
            fs: self,
            path,
            mount_point,
        }
    }

    /// Format the filesystem.
    fn format<'a>(&'a self) -> FormatFuture<'a, Self>
    where
        Self: Unpin,
    {
        FormatFuture { fs: self }
    }

    /// Get the attributes of the filesystem.
    fn statfs<'a>(&'a self) -> StatFsFuture<'a, Self>
    where
        Self: Unpin,
    {
        StatFsFuture { fs: self }
    }

    /// Get the root directory of the filesystem.
    fn root_dir<'a>(&'a self) -> RootDirFuture<'a, Self>
    where
        Self: Unpin,
    {
        RootDirFuture { fs: self }
    }
}

impl<T: VfsOps + ?Sized> AsyncVfsOps for T {}
