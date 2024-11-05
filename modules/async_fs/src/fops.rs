//! Low-level filesystem operations.

use axerrno::{ax_err, ax_err_type, AxResult};
use async_vfs::{AsyncVfsNodeOps, VfsError, VfsNodeOps, VfsNodeRef};
use async_io::{AsyncRead, AsyncSeek, AsyncWrite, SeekFrom};
use capability::{Cap, WithCap};
use core::fmt;
use core::pin::Pin;
use core::task::{Context, Poll};

#[cfg(feature = "myfs")]
pub use crate::dev::Disk;
#[cfg(feature = "myfs")]
pub use crate::fs::myfs::MyFileSystemIf;

/// Alias of [`axfs_vfs::VfsNodeType`].
pub type FileType = async_vfs::VfsNodeType;
/// Alias of [`axfs_vfs::VfsDirEntry`].
pub type DirEntry = async_vfs::VfsDirEntry;
/// Alias of [`axfs_vfs::VfsNodeAttr`].
pub type FileAttr = async_vfs::VfsNodeAttr;
/// Alias of [`axfs_vfs::VfsNodePerm`].
pub type FilePerm = async_vfs::VfsNodePerm;

/// An opened file object, with open permissions and a cursor.
#[derive(Clone)]
pub struct File {
    node: WithCap<VfsNodeRef>,
    is_append: bool,
    offset: u64,
}

impl AsyncRead for File {
    fn read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<AxResult<usize>> {
        let Self { node, offset, .. } = self.get_mut();
        let node = node.access(Cap::READ)?;
        let read_len = futures_core::ready!(
            VfsNodeOps::poll_read_at(Pin::new(node), cx, *offset, buf)
        )?;
        *offset += read_len as u64;
        Poll::Ready(Ok(read_len))
    }
}

impl AsyncWrite for File {
    fn write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<AxResult<usize>> {
        let Self { node, is_append, offset } = self.get_mut();
        let node = node.access(Cap::WRITE)?;
        if *is_append {
            let attr = futures_core::ready!(VfsNodeOps::poll_get_attr(Pin::new(node), cx)).unwrap();
            *offset = attr.size();
        };
        let write_len = futures_core::ready!(
            VfsNodeOps::poll_write_at(Pin::new(node), cx, *offset, buf)
        ).unwrap();
        *offset += write_len as u64;
        Poll::Ready(Ok(write_len))
    }

    fn flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<AxResult<()>> {
        let node = self.get_mut().node.access(Cap::WRITE)?;
        VfsNodeOps::poll_fsync(Pin::new(node), cx)
    }

    fn close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<AxResult<()>> {
        self.flush(cx)
    }
}

impl AsyncSeek for File {
    fn seek(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        pos: SeekFrom,
    ) -> Poll<AxResult<u64>> {
        let Self { 
            node, 
            offset, .. 
        } = self.get_mut();
        let node = node.access(Cap::empty())?;
        let attr = futures_core::ready!(VfsNodeOps::poll_get_attr(Pin::new(node), cx)).unwrap();
        let size = attr.size();
        let new_offset = match pos {
            SeekFrom::Start(pos) => Some(pos),
            SeekFrom::Current(off) => offset.checked_add_signed(off),
            SeekFrom::End(off) => size.checked_add_signed(off),
        }.ok_or_else(|| ax_err_type!(InvalidInput))?;
        *offset = new_offset;
        Poll::Ready(Ok(new_offset))
    }
}

/// An opened directory object, with open permissions and a cursor for
/// [`read_dir`](Directory::read_dir).
pub struct Directory {
    node: WithCap<VfsNodeRef>,
    entry_idx: usize,
}

/// Options and flags which can be used to configure how a file is opened.
#[derive(Clone)]
pub struct OpenOptions {
    // generic
    read: bool,
    write: bool,
    append: bool,
    truncate: bool,
    create: bool,
    create_new: bool,
    // system-specific
    _custom_flags: i32,
    _mode: u32,
}

impl OpenOptions {
    /// Creates a blank new set of options ready for configuration.
    pub const fn new() -> Self {
        Self {
            // generic
            read: false,
            write: false,
            append: false,
            truncate: false,
            create: false,
            create_new: false,
            // system-specific
            _custom_flags: 0,
            _mode: 0o666,
        }
    }
    /// Sets the option for read access.
    pub fn read(&mut self, read: bool) {
        self.read = read;
    }
    /// Sets the option for write access.
    pub fn write(&mut self, write: bool) {
        self.write = write;
    }
    /// Sets the option for the append mode.
    pub fn append(&mut self, append: bool) {
        self.append = append;
    }
    /// Sets the option for truncating a previous file.
    pub fn truncate(&mut self, truncate: bool) {
        self.truncate = truncate;
    }
    /// Sets the option to create a new file, or open it if it already exists.
    pub fn create(&mut self, create: bool) {
        self.create = create;
    }
    /// Sets the option to create a new file, failing if it already exists.
    pub fn create_new(&mut self, create_new: bool) {
        self.create_new = create_new;
    }

    const fn is_valid(&self) -> bool {
        if !self.read && !self.write && !self.append {
            return false;
        }
        match (self.write, self.append) {
            (true, false) => {}
            (false, false) => {
                if self.truncate || self.create || self.create_new {
                    return false;
                }
            }
            (_, true) => {
                if self.truncate && !self.create_new {
                    return false;
                }
            }
        }
        true
    }
}

impl File {
    async fn _open_at(dir: Option<&VfsNodeRef>, path: &str, opts: &OpenOptions) -> AxResult<Self> {
        debug!("open file: {} {:?}", path, opts);
        if !opts.is_valid() {
            return ax_err!(InvalidInput);
        }

        let node_option = crate::root::lookup(dir, path).await;
        let node = if opts.create || opts.create_new {
            match node_option {
                Ok(node) => {
                    // already exists
                    if opts.create_new {
                        return ax_err!(AlreadyExists);
                    }
                    node
                }
                // not exists, create new
                Err(VfsError::NotFound) => crate::root::create_file(dir, path).await?,
                Err(e) => return Err(e),
            }
        } else {
            // just open the existing
            node_option?
        };
        let attr = node.get_attr().await?;
        if attr.is_dir()
            && (opts.create || opts.create_new || opts.write || opts.append || opts.truncate)
        {
            return ax_err!(IsADirectory);
        }
        let access_cap = opts.into();
        if !perm_to_cap(attr.perm()).contains(access_cap) {
            return ax_err!(PermissionDenied);
        }
        node.open().await?;
        if opts.truncate {
            node.truncate(0).await?;
        }
        Ok(Self {
            node: WithCap::new(node, access_cap),
            is_append: opts.append,
            offset: 0,
        })
    }

    /// Opens a file at the path relative to the current directory. Returns a
    /// [`File`] object.
    pub async fn open_withperm(path: &str, opts: &OpenOptions) -> AxResult<Self> {
        Self::_open_at(None, path, opts).await
    }

    /// Truncates the file to the specified size.
    pub async fn truncate(&self, size: u64) -> AxResult {
        self.node.access(Cap::WRITE)?.truncate(size).await?;
        Ok(())
    }

    /// Reads the file at the current position. Returns the number of bytes
    /// read.
    ///
    /// After the read, the cursor will be advanced by the number of bytes read.
    pub async fn read(&mut self, buf: &mut [u8]) -> AxResult<usize> {
        let node = self.node.access(Cap::READ)?;
        let read_len = node.read_at(self.offset, buf).await?;
        self.offset += read_len as u64;
        Ok(read_len)
    }

    /// Reads the file at the given position. Returns the number of bytes read.
    ///
    /// It does not update the file cursor.
    pub async fn read_at(&self, offset: u64, buf: &mut [u8]) -> AxResult<usize> {
        let node = self.node.access(Cap::READ)?;
        let read_len = node.read_at(offset, buf).await?;
        Ok(read_len)
    }

    /// Writes the file at the current position. Returns the number of bytes
    /// written.
    ///
    /// After the write, the cursor will be advanced by the number of bytes
    /// written.
    pub async fn write(&mut self, buf: &[u8]) -> AxResult<usize> {
        let node = self.node.access(Cap::WRITE)?;
        if self.is_append {
            self.offset = self.get_attr().await?.size();
        };
        let write_len = node.write_at(self.offset, buf).await?;
        self.offset += write_len as u64;
        Ok(write_len)
    }

    /// Writes the file at the given position. Returns the number of bytes
    /// written.
    ///
    /// It does not update the file cursor.
    pub async fn write_at(&self, offset: u64, buf: &[u8]) -> AxResult<usize> {
        let node = self.node.access(Cap::WRITE)?;
        let write_len = node.write_at(offset, buf).await?;
        Ok(write_len)
    }

    /// Flushes the file, writes all buffered data to the underlying device.
    pub async fn flush(&self) -> AxResult {
        self.node.access(Cap::WRITE)?.fsync().await?;
        Ok(())
    }

    /// Sets the cursor of the file to the specified offset. Returns the new
    /// position after the seek.
    pub async fn seek(&mut self, pos: SeekFrom) -> AxResult<u64> {
        let size = self.get_attr().await?.size();
        let new_offset = match pos {
            SeekFrom::Start(pos) => Some(pos),
            SeekFrom::Current(off) => self.offset.checked_add_signed(off),
            SeekFrom::End(off) => size.checked_add_signed(off),
        }
        .ok_or_else(|| ax_err_type!(InvalidInput))?;
        self.offset = new_offset;
        Ok(new_offset)
    }

    /// Gets the file attributes.
    pub async fn get_attr(&self) -> AxResult<FileAttr> {
        self.node.access(Cap::empty())?.get_attr().await
    }

    #[allow(unused)]
    /// whether the file is readable.
    pub fn readable(&self) -> bool {
        self.node.can_access(Cap::READ)
    }

    #[allow(unused)]
    /// whether the file is writable.
    pub fn writable(&self) -> bool {
        self.node.can_access(Cap::WRITE)
    }

    #[allow(unused)]
    /// whether the file is executable.
    pub fn executable(&self) -> bool {
        self.node.can_access(Cap::EXECUTE)
    }
}

impl Directory {
    async fn _open_dir_at(dir: Option<&VfsNodeRef>, path: &str, opts: &OpenOptions) -> AxResult<Self> {
        debug!("open dir: {}", path);
        if !opts.read {
            return ax_err!(InvalidInput);
        }
        if opts.create || opts.create_new || opts.write || opts.append || opts.truncate {
            return ax_err!(InvalidInput);
        }

        let node = crate::root::lookup(dir, path).await?;
        let attr = node.get_attr().await?;
        if !attr.is_dir() {
            return ax_err!(NotADirectory);
        }
        let access_cap = opts.into();
        if !perm_to_cap(attr.perm()).contains(access_cap) {
            return ax_err!(PermissionDenied);
        }

        node.open().await?;
        Ok(Self {
            node: WithCap::new(node, access_cap),
            entry_idx: 0,
        })
    }

    fn access_at(&self, path: &str) -> AxResult<Option<&VfsNodeRef>> {
        if path.starts_with('/') {
            Ok(None)
        } else {
            Ok(Some(self.node.access(Cap::EXECUTE)?))
        }
    }

    /// Opens a directory at the path relative to the current directory.
    /// Returns a [`Directory`] object.
    pub async fn open_dir(path: &str, opts: &OpenOptions) -> AxResult<Self> {
        Self::_open_dir_at(None, path, opts).await
    }

    /// Opens a directory at the path relative to this directory. Returns a
    /// [`Directory`] object.
    pub async fn open_dir_at(&self, path: &str, opts: &OpenOptions) -> AxResult<Self> {
        Self::_open_dir_at(self.access_at(path)?, path, opts).await
    }

    /// Opens a file at the path relative to this directory. Returns a [`File`]
    /// object.
    pub async fn open_file_at(&self, path: &str, opts: &OpenOptions) -> AxResult<File> {
        File::_open_at(self.access_at(path)?, path, opts).await
    }

    /// Creates an empty file at the path relative to this directory.
    pub async fn create_file(&self, path: &str) -> AxResult<VfsNodeRef> {
        crate::root::create_file(self.access_at(path)?, path).await
    }

    /// Creates an empty directory at the path relative to this directory.
    pub async fn create_dir(&self, path: &str) -> AxResult {
        crate::root::create_dir(self.access_at(path)?, path).await
    }

    /// Removes a file at the path relative to this directory.
    pub async fn remove_file(&self, path: &str) -> AxResult {
        crate::root::remove_file(self.access_at(path)?, path).await
    }

    /// Removes a directory at the path relative to this directory.
    pub async fn remove_dir(&self, path: &str) -> AxResult {
        crate::root::remove_dir(self.access_at(path)?, path).await
    }

    /// Reads directory entries starts from the current position into the
    /// given buffer. Returns the number of entries read.
    ///
    /// After the read, the cursor will be advanced by the number of entries
    /// read.
    pub async fn read_dir(&mut self, dirents: &mut [DirEntry]) -> AxResult<usize> {
        let n = self
            .node
            .access(Cap::READ)?
            .read_dir(self.entry_idx, dirents).await?;
        self.entry_idx += n;
        Ok(n)
    }

    pub fn poll_read_dir(
        self: Pin<&mut Self>, 
        cx: &mut Context<'_>, 
        dirents: &mut [DirEntry]
    ) -> Poll<AxResult<usize>> {
        let Self { node, entry_idx } = self.get_mut();
        let node = node.access(Cap::READ)?;
        let n = futures_core::ready!(VfsNodeOps::poll_read_dir(Pin::new(node), cx, *entry_idx, dirents))?;
        *entry_idx += n;
        Poll::Ready(Ok(n))
    }

    /// Rename a file or directory to a new name.
    /// Delete the original file if `old` already exists.
    ///
    /// This only works then the new path is in the same mounted fs.
    pub async fn rename(&self, old: &str, new: &str) -> AxResult {
        crate::root::rename(old, new).await
    }
}

impl Drop for File {
    fn drop(&mut self) {
        unsafe { self.node.access_unchecked().release().ok() };
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        unsafe { self.node.access_unchecked().release().ok() };
    }
}

impl fmt::Debug for OpenOptions {
    #[allow(unused_assignments)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut written = false;
        macro_rules! fmt_opt {
            ($field: ident, $label: literal) => {
                if self.$field {
                    if written {
                        write!(f, " | ")?;
                    }
                    write!(f, $label)?;
                    written = true;
                }
            };
        }
        fmt_opt!(read, "READ");
        fmt_opt!(write, "WRITE");
        fmt_opt!(append, "APPEND");
        fmt_opt!(truncate, "TRUNC");
        fmt_opt!(create, "CREATE");
        fmt_opt!(create_new, "CREATE_NEW");
        Ok(())
    }
}

impl From<&OpenOptions> for Cap {
    fn from(opts: &OpenOptions) -> Cap {
        let mut cap = Cap::empty();
        if opts.read {
            cap |= Cap::READ;
        }
        if opts.write | opts.append {
            cap |= Cap::WRITE;
        }
        cap
    }
}

fn perm_to_cap(perm: FilePerm) -> Cap {
    let mut cap = Cap::empty();
    if perm.owner_readable() {
        cap |= Cap::READ;
    }
    if perm.owner_writable() {
        cap |= Cap::WRITE;
    }
    if perm.owner_executable() {
        cap |= Cap::EXECUTE;
    }
    cap
}
