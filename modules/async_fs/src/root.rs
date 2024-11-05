//! Root directory of the filesystem
//!
//! TODO: it doesn't work very well if the mount points have containment relationships.

use alloc::{
    string::{String, ToString},
    sync::Arc,
    vec::Vec,
};
use axerrno::{ax_err, AxError, AxResult};
use async_vfs::{AsyncVfsNodeOps, AsyncVfsOps, VfsNodeAttr, VfsNodeOps, VfsNodeRef, VfsNodeType, VfsOps, VfsResult};
use sync::Mutex;
use lazy_init::LazyInit;
use core::pin::Pin;
use core::task::{Context, Poll};

use crate::{api::FileType, fs};

static CURRENT_DIR_PATH: Mutex<String> = Mutex::new(String::new());
static CURRENT_DIR: LazyInit<Mutex<VfsNodeRef>> = LazyInit::new();

struct MountPoint {
    path: &'static str,
    fs: Arc<dyn VfsOps + Unpin>,
}

struct RootDirectory {
    main_fs: Arc<dyn VfsOps + Unpin>,
    mounts: Vec<MountPoint>,
}

static ROOT_DIR: LazyInit<Arc<RootDirectory>> = LazyInit::new();

impl MountPoint {
    #[allow(unused)]
    pub fn new(path: &'static str, fs: Arc<dyn VfsOps + Unpin>) -> Self {
        Self { path, fs }
    }
}

impl Drop for MountPoint {
    fn drop(&mut self) {
        self.fs.umount().unwrap();
    }
}

impl RootDirectory {
    pub const fn new(main_fs: Arc<dyn VfsOps + Unpin>) -> Self {
        Self {
            main_fs,
            mounts: Vec::new(),
        }
    }

    #[allow(unused)]
    pub async fn mount(&mut self, path: &'static str, fs: Arc<dyn VfsOps + Unpin>) -> AxResult {
        if path == "/" {
            return ax_err!(InvalidInput, "cannot mount root filesystem");
        }
        if !path.starts_with('/') {
            return ax_err!(InvalidInput, "mount path must start with '/'");
        }
        if self.mounts.iter().any(|mp| mp.path == path) {
            return ax_err!(InvalidInput, "mount point already exists");
        }
        // create the mount point in the main filesystem if it does not exist
        self.main_fs.root_dir().await.create(path, FileType::Dir).await?;
        fs.mount(path, &self.main_fs.root_dir().await.lookup(path).await?).await?;
        self.mounts.push(MountPoint::new(path, fs));
        Ok(())
    }

    pub fn _umount(&mut self, path: &str) {
        self.mounts.retain(|mp| mp.path != path);
    }

    pub fn contains(&self, path: &str) -> bool {
        self.mounts.iter().any(|mp| mp.path == path)
    }

    fn lookup_mounted_fs<F, T>(&self, path: &str, f: F) -> Poll<AxResult<T>>
    where
        F: FnOnce(Arc<dyn VfsOps + Unpin>, &str) -> Poll<AxResult<T>>,
    {
        debug!("lookup at root: {}", path);
        let path = path.trim_matches('/');
        if let Some(rest) = path.strip_prefix("./") {
            return self.lookup_mounted_fs(rest, f);
        }

        let mut idx = 0;
        let mut max_len = 0;

        // Find the filesystem that has the longest mounted path match
        // TODO: more efficient, e.g. trie

        for (i, mp) in self.mounts.iter().enumerate() {
            // skip the first '/'
            // two conditions
            // 1. path == mp.path, e.g. dev
            // 2. path == mp.path + '/', e.g. dev/
            let prev = mp.path[1..].to_string() + "/";
            if path.starts_with(&mp.path[1..])
                && (path.len() == prev.len() - 1 || path.starts_with(&prev))
                && prev.len() > max_len
            {
                max_len = mp.path.len() - 1;
                idx = i;
            }
        }
        if max_len == 0 {
            f(self.main_fs.clone(), path) // not matched any mount point
        } else {
            f(self.mounts[idx].fs.clone(), &path[max_len..]) // matched at `idx`
        }
    }
}

impl VfsNodeOps for RootDirectory {
    async_vfs::impl_vfs_dir_default! {}

    fn poll_get_attr(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<VfsResult<VfsNodeAttr>> {
        let root_dir = core::task::ready!(
            VfsOps::poll_root_dir(Pin::new(&self.main_fs), cx)
        );
        VfsNodeOps::poll_get_attr(Pin::new(&root_dir), cx)    
    }

    fn poll_lookup(self: Pin<&Self>, cx: &mut Context<'_>, _path: &str) -> Poll<VfsResult<VfsNodeRef>> {
        self.lookup_mounted_fs(_path, |fs, rest_path| {
            let root_dir = core::task::ready!(
                VfsOps::poll_root_dir(Pin::new(&fs), cx)
            );
            VfsNodeOps::poll_lookup(Pin::new(&root_dir), cx, rest_path)
        })
    }

    fn poll_create(self: Pin<&Self>, cx: &mut Context<'_>, path: &str, ty: VfsNodeType) -> Poll<VfsResult> {
        self.lookup_mounted_fs(path, |fs, rest_path| {
            if rest_path.is_empty() {
                Poll::Ready(Ok(())) // already exists
            } else {
                let root_dir = core::task::ready!(
                    VfsOps::poll_root_dir(Pin::new(&fs), cx)
                );
                VfsNodeOps::poll_create(Pin::new(&root_dir), cx, rest_path, ty)
            }
        })
    }

    fn poll_remove(self: Pin<&Self>, cx: &mut Context<'_>, path: &str) -> Poll<VfsResult> {
        self.lookup_mounted_fs(path, |fs, rest_path| {
            if rest_path.is_empty() {
                Poll::Ready(ax_err!(PermissionDenied)) // cannot remove mount points
            } else {
                let root_dir = core::task::ready!(
                    VfsOps::poll_root_dir(Pin::new(&fs), cx)
                );
                VfsNodeOps::poll_remove(Pin::new(&root_dir), cx, rest_path)
            }
        })
    }

    fn poll_rename(
        self: Pin<&Self>, 
        cx: &mut Context<'_>, 
        src_path: &str, 
        dst_path: &str
    ) -> Poll<VfsResult> {
        self.lookup_mounted_fs(src_path, |fs, rest_path| {
            if rest_path.is_empty() {
                Poll::Ready(ax_err!(PermissionDenied)) // cannot rename mount points
            } else {
                let root_dir = core::task::ready!(
                    VfsOps::poll_root_dir(Pin::new(&fs), cx)
                );
                VfsNodeOps::poll_rename(Pin::new(&root_dir), cx, rest_path, dst_path)
            }
        })
    }
}

pub(crate) async fn init_rootfs(disk: crate::dev::Disk) {
    cfg_if::cfg_if! {
        if #[cfg(feature = "myfs")] { // override the default filesystem
            let main_fs = fs::myfs::new_myfs(disk);
        } else if #[cfg(feature = "lwext4_rust")] {
            static EXT4_FS: LazyInit<Arc<fs::lwext4_rust::Ext4FileSystem>> = LazyInit::new();
            EXT4_FS.init_by(Arc::new(fs::lwext4_rust::Ext4FileSystem::new(disk)));
            let main_fs = EXT4_FS.clone();
        } else if #[cfg(feature = "ext4_rs")] {
            static EXT4_FS: LazyInit<Arc<fs::ext4_rs::Ext4FileSystem>> = LazyInit::new();
            EXT4_FS.init_by(Arc::new(fs::ext4_rs::Ext4FileSystem::new(disk)));
            let main_fs = EXT4_FS.clone();
        } else if #[cfg(feature = "another_ext4")] {
            static EXT4_FS: LazyInit<Arc<fs::another_ext4::Ext4FileSystem>> = LazyInit::new();
            EXT4_FS.init_by(Arc::new(fs::another_ext4::Ext4FileSystem::new(disk)));
            let main_fs = EXT4_FS.clone();
        } else if #[cfg(feature = "fatfs")] {
            // default to be fatfs
            static FAT_FS: LazyInit<Arc<fs::fatfs::FatFileSystem>> = LazyInit::new();
            FAT_FS.init_by(Arc::new(fs::fatfs::FatFileSystem::new(disk)));
            FAT_FS.init();
            let main_fs = FAT_FS.clone();
        }
    }
    let root_dir = RootDirectory::new(main_fs);

    ROOT_DIR.init_by(Arc::new(root_dir));
    CURRENT_DIR.init_by(Mutex::new(ROOT_DIR.clone()));
    *CURRENT_DIR_PATH.lock().await = "/".into();
}

async fn parent_node_of(dir: Option<&VfsNodeRef>, path: &str) -> VfsNodeRef {
    if path.starts_with('/') {
        ROOT_DIR.clone()
    } else {
        if dir.is_none() {
            CURRENT_DIR.lock().await.clone()
        } else {
            dir.cloned().unwrap()
        }
    }
}

pub(crate) async fn absolute_path(path: &str) -> AxResult<String> {
    if path.starts_with('/') {
        Ok(async_vfs::path::canonicalize(path))
    } else {
        let path = CURRENT_DIR_PATH.lock().await.clone() + path;
        Ok(async_vfs::path::canonicalize(&path))
    }
}

pub(crate) async fn lookup(dir: Option<&VfsNodeRef>, path: &str) -> AxResult<VfsNodeRef> {
    if path.is_empty() {
        return ax_err!(NotFound);
    }
    let node = parent_node_of(dir, path).await.lookup(path).await?;
    if path.ends_with('/') && !node.get_attr().await?.is_dir() {
        ax_err!(NotADirectory)
    } else {
        Ok(node)
    }
}

pub(crate) async fn create_file(dir: Option<&VfsNodeRef>, path: &str) -> AxResult<VfsNodeRef> {
    if path.is_empty() {
        return ax_err!(NotFound);
    } else if path.ends_with('/') {
        return ax_err!(NotADirectory);
    }
    let parent = parent_node_of(dir, path).await;
    parent.create(path, VfsNodeType::File).await?;
    parent.lookup(path).await
}

pub(crate) async fn create_dir(dir: Option<&VfsNodeRef>, path: &str) -> AxResult {
    match lookup(dir, path).await {
        Ok(_) => ax_err!(AlreadyExists),
        Err(AxError::NotFound) => parent_node_of(dir, path).await.create(path, VfsNodeType::Dir).await,
        Err(e) => Err(e),
    }
}

pub(crate) async fn remove_file(dir: Option<&VfsNodeRef>, path: &str) -> AxResult {
    let node = lookup(dir, path).await?;
    let attr = node.get_attr().await?;
    if attr.is_dir() {
        ax_err!(IsADirectory)
    } else if !attr.perm().owner_writable() {
        ax_err!(PermissionDenied)
    } else {
        parent_node_of(dir, path).await.remove(path).await
    }
}

pub(crate) async fn remove_dir(dir: Option<&VfsNodeRef>, path: &str) -> AxResult {
    if path.is_empty() {
        return ax_err!(NotFound);
    }
    let path_check = path.trim_matches('/');
    if path_check.is_empty() {
        return ax_err!(DirectoryNotEmpty); // rm -d '/'
    } else if path_check == "."
        || path_check == ".."
        || path_check.ends_with("/.")
        || path_check.ends_with("/..")
    {
        return ax_err!(InvalidInput);
    }
    if ROOT_DIR.contains(&absolute_path(path).await?) {
        return ax_err!(PermissionDenied);
    }

    let node = lookup(dir, path).await?;
    let attr = node.get_attr().await?;
    if !attr.is_dir() {
        ax_err!(NotADirectory)
    } else if !attr.perm().owner_writable() {
        ax_err!(PermissionDenied)
    } else {
        parent_node_of(dir, path).await.remove(path).await
    }
}

pub(crate) async fn current_dir() -> AxResult<String> {
    Ok(CURRENT_DIR_PATH.lock().await.clone())
}

pub(crate) async fn set_current_dir(path: &str) -> AxResult {
    let mut abs_path = absolute_path(path).await?;
    if !abs_path.ends_with('/') {
        abs_path += "/";
    }
    if abs_path == "/" {
        *CURRENT_DIR.lock().await = ROOT_DIR.clone();
        *CURRENT_DIR_PATH.lock().await = "/".into();
        return Ok(());
    }

    let node = lookup(None, &abs_path).await?;
    let attr = node.get_attr().await?;
    if !attr.is_dir() {
        ax_err!(NotADirectory)
    } else if !attr.perm().owner_executable() {
        ax_err!(PermissionDenied)
    } else {
        *CURRENT_DIR.lock().await = node;
        *CURRENT_DIR_PATH.lock().await = abs_path;
        Ok(())
    }
}

pub(crate) async fn rename(old: &str, new: &str) -> AxResult {
    if parent_node_of(None, new).await.lookup(new).await.is_ok() {
        warn!("dst file already exist, now remove it");
        remove_file(None, new).await?;
    }
    parent_node_of(None, old).await.rename(old, new).await
}
