//! [`std::fs`]-like high-level filesystem manipulation operations.

mod dir;
mod file;

pub mod port;

pub use self::dir::{DirBuilder, DirEntry, ReadDir};
pub use self::file::{File, FileType, Metadata, OpenOptions, Permissions};
use axerrno::AxResult;
use async_vfs::VfsNodeRef;
pub use async_io::{Read, Seek, SeekFrom, Write, Result};
pub use port::*;

use alloc::{string::String, vec::Vec};

/// Returns an iterator over the entries within a directory.
pub async fn read_dir(path: &str) -> Result<ReadDir> {
    ReadDir::new(path).await
}

/// Returns the canonical, absolute form of a path with all intermediate
/// components normalized.
pub async fn canonicalize(path: &str) -> Result<String> {
    crate::root::absolute_path(path).await
}

/// Returns the current working directory as a [`String`].
pub async fn current_dir() -> Result<String> {
    crate::root::current_dir().await
}

/// Changes the current working directory to the specified path.
pub async fn set_current_dir(path: &str) -> Result<()> {
    crate::root::set_current_dir(path).await
}

/// Read the entire contents of a file into a bytes vector.
pub async fn read(path: &str) -> Result<Vec<u8>> {
    let mut file = File::open(path).await?;
    let size = file.metadata().await.map(|m| m.len()).unwrap_or(0);
    let mut bytes = Vec::with_capacity(size as usize);
    file.read_to_end(&mut bytes).await?;
    Ok(bytes)
}

/// Read the entire contents of a file into a string.
pub async fn read_to_string(path: &str) -> Result<String> {
    let mut file = File::open(path).await?;
    let size = file.metadata().await.map(|m| m.len()).unwrap_or(0);
    let mut string = String::with_capacity(size as usize);
    file.read_to_string(&mut string).await?;
    Ok(string)
}

/// Write a slice as the entire contents of a file.
pub async fn write<C: AsRef<[u8]>>(path: &str, contents: C) -> Result<()> {
    File::create(path).await?.write_all(contents.as_ref()).await
}

/// Given a path, query the file system to get information about a file,
/// directory, etc.
pub async fn metadata(path: &str) -> Result<Metadata> {
    File::open(path).await?.metadata().await
}

/// Creates a new, empty directory at the provided path.
pub async fn create_dir(path: &str) -> Result<()> {
    DirBuilder::new().create(path).await
}

/// Recursively create a directory and all of its parent components if they
/// are missing.
pub async fn create_dir_all(path: &str) -> Result<()> {
    DirBuilder::new().recursive(true).create(path).await
}

/// Removes an empty directory.
pub async fn remove_dir(path: &str) -> Result<()> {
    crate::root::remove_dir(None, path).await
}

/// Removes a file from the filesystem.
pub async fn remove_file(path: &str) -> Result<()> {
    crate::root::remove_file(None, path).await
}

/// Rename a file or directory to a new name.
/// Delete the original file if `old` already exists.
///
/// This only works then the new path is in the same mounted fs.
pub async fn rename(old: &str, new: &str) -> Result<()> {
    crate::root::rename(old, new).await
}

/// Check if a path exists.
pub async fn path_exists(path: &str) -> bool {
    crate::root::lookup(None, path).await.is_ok()
}

/// Look up a file by a given path.
pub async fn lookup(path: &str) -> AxResult<VfsNodeRef> {
    crate::root::lookup(None, path).await
}
