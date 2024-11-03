extern crate alloc;

use alloc::string::String;
use core::fmt;

use super::FileType;
use crate::io::Result;
use async_io::AsyncStream;

use aos_api::fs as api;

/// Iterator over the entries in a directory.
pub struct ReadDir<'a> {
    path: &'a str,
    inner: api::AxDirHandle,
    buf_pos: usize,
    buf_end: usize,
    end_of_stream: bool,
    dirent_buf: [api::AxDirEntry; 31],
}

/// Entries returned by the [`ReadDir`] iterator.
pub struct DirEntry<'a> {
    dir_path: &'a str,
    entry_name: String,
    entry_type: FileType,
}

/// A builder used to create directories in various manners.
#[derive(Default, Debug)]
pub struct DirBuilder {
    recursive: bool,
}

impl<'a> ReadDir<'a> {
    pub(super) async fn new(path: &'a str) -> Result<Self> {
        let mut opts = api::AxOpenOptions::new();
        opts.read(true);
        let inner = api::ax_open_dir(path, &opts).await?;

        const EMPTY: api::AxDirEntry = api::AxDirEntry::default();
        let dirent_buf = [EMPTY; 31];
        Ok(ReadDir {
            path,
            inner,
            end_of_stream: false,
            buf_pos: 0,
            buf_end: 0,
            dirent_buf,
        })
    }
}

use core::{
    pin::Pin,
    task::{Context, Poll},
};

impl<'a> AsyncStream for ReadDir<'a> {
    type Item = Result<DirEntry<'a>>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if self.end_of_stream {
            return Poll::Ready(None);
        }
        let Self { 
            path, 
            inner, 
            buf_pos, 
            buf_end, 
            end_of_stream, 
            dirent_buf 
        } = self.get_mut();
        loop {
            if *buf_pos >= *buf_end {
                match futures_core::ready!(Pin::new(&mut *inner).poll_read_dir(cx, dirent_buf)) {
                    Ok(n) => {
                        if n == 0 {
                            *end_of_stream = true;
                            return Poll::Ready(None);
                        }
                        *buf_pos = 0;
                        *buf_end = n;
                    }
                    Err(e) => {
                        *end_of_stream = true;
                        return Poll::Ready(Some(Err(e)));
                    }
                }
            }
            let entry = &dirent_buf[*buf_pos];
            *buf_pos += 1;
            let name_bytes = entry.name_as_bytes();
            if name_bytes == b"." || name_bytes == b".." {
                continue;
            }
            let entry_name = unsafe { core::str::from_utf8_unchecked(name_bytes).into() };
            let entry_type = entry.entry_type();
            return Poll::Ready(Some(Ok(DirEntry {
                dir_path: path,
                entry_name,
                entry_type,
            })));
        }
    }
    
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

impl<'a> DirEntry<'a> {
    /// Returns the full path to the file that this entry represents.
    ///
    /// The full path is created by joining the original path to `read_dir`
    /// with the filename of this entry.
    pub fn path(&self) -> String {
        String::from(self.dir_path.trim_end_matches('/')) + "/" + &self.entry_name
    }

    /// Returns the bare file name of this directory entry without any other
    /// leading path component.
    pub fn file_name(&self) -> String {
        self.entry_name.clone()
    }

    /// Returns the file type for the file that this entry points at.
    pub fn file_type(&self) -> FileType {
        self.entry_type
    }
}

impl fmt::Debug for DirEntry<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("DirEntry").field(&self.path()).finish()
    }
}

impl DirBuilder {
    /// Creates a new set of options with default mode/security settings for all
    /// platforms and also non-recursive.
    pub fn new() -> Self {
        Self { recursive: false }
    }

    /// Indicates that directories should be created recursively, creating all
    /// parent directories. Parents that do not exist are created with the same
    /// security and permissions settings.
    pub fn recursive(&mut self, recursive: bool) -> &mut Self {
        self.recursive = recursive;
        self
    }

    /// Creates the specified directory with the options configured in this
    /// builder.
    pub async fn create(&self, path: &str) -> Result<()> {
        if self.recursive {
            self.create_dir_all(path)
        } else {
            api::ax_create_dir(path).await
        }
    }

    fn create_dir_all(&self, _path: &str) -> Result<()> {
        axerrno::ax_err!(
            Unsupported,
            "Recursive directory creation is not supported yet"
        )
    }
}
