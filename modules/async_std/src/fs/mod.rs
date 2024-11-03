//! Filesystem manipulation operations.

mod dir;
mod file;

use crate::io;
use async_io::Write;

#[cfg(feature = "alloc")]
use alloc::{string::String, vec::Vec};

pub use self::dir::{DirBuilder, DirEntry, ReadDir};
pub use self::file::{File, FileType, Metadata, OpenOptions, Permissions};

/// Read the entire contents of a file into a bytes vector.
#[cfg(feature = "alloc")]
pub async fn read(path: &str) -> io::Result<Vec<u8>> {
    let mut file = File::open(path).await?;
    let size = file.metadata().await.map(|m| m.len()).unwrap_or(0);
    let mut bytes = Vec::with_capacity(size as usize);
    file.read_to_end(&mut bytes).await?;
    Ok(bytes)
}

/// Read the entire contents of a file into a string.
#[cfg(feature = "alloc")]
pub async fn read_to_string(path: &str) -> io::Result<String> {
    let mut file = File::open(path).await?;
    let size = file.metadata().await.map(|m| m.len()).unwrap_or(0);
    let mut string = String::with_capacity(size as usize);
    file.read_to_string(&mut string).await?;
    Ok(string)
}

/// Write a slice as the entire contents of a file.
pub async fn write<C: AsRef<[u8]>>(path: &str, contents: C) -> io::Result<()> {
    File::create(path).await?.write_all(contents.as_ref()).await
}

/// Given a path, query the file system to get information about a file,
/// directory, etc.
pub async fn metadata(path: &str) -> io::Result<Metadata> {
    File::open(path).await?.metadata().await
}

/// Returns an iterator over the entries within a directory.
pub async fn read_dir(path: &str) -> io::Result<ReadDir> {
    ReadDir::new(path).await
}

/// Creates a new, empty directory at the provided path.
pub async fn create_dir(path: &str) -> io::Result<()> {
    DirBuilder::new().create(path).await
}

/// Recursively create a directory and all of its parent components if they
/// are missing.
pub async fn create_dir_all(path: &str) -> io::Result<()> {
    DirBuilder::new().recursive(true).create(path).await
}

/// Removes an empty directory.
pub async fn remove_dir(path: &str) -> io::Result<()> {
    aos_api::fs::ax_remove_dir(path).await
}

/// Removes a file from the filesystem.
pub async fn remove_file(path: &str) -> io::Result<()> {
    aos_api::fs::ax_remove_file(path).await
}

/// Rename a file or directory to a new name.
/// Delete the original file if `old` already exists.
///
/// This only works then the new path is in the same mounted fs.
pub async fn rename(old: &str, new: &str) -> io::Result<()> {
    aos_api::fs::ax_rename(old, new).await
}
