use alloc::string::String;
use axerrno::AxResult;
use async_fs::fops::{Directory, File};

pub use async_fs::fops::DirEntry as AxDirEntry;
pub use async_fs::fops::FileAttr as AxFileAttr;
pub use async_fs::fops::FilePerm as AxFilePerm;
pub use async_fs::fops::FileType as AxFileType;
pub use async_fs::fops::OpenOptions as AxOpenOptions;
pub use async_io::SeekFrom as AxSeekFrom;
use async_io::{AsyncRead, AsyncWrite, AsyncSeek};

#[cfg(feature = "myfs")]
pub use axfs::fops::{Disk as AxDisk, MyFileSystemIf};

/// A handle to an opened file.
pub struct AxFileHandle(File);

/// A handle to an opened directory.
pub struct AxDirHandle(Directory);

pub async fn ax_open_file(path: &str, opts: &AxOpenOptions) -> AxResult<AxFileHandle> {
    Ok(AxFileHandle(File::open_withperm(path, opts).await?))
}

pub async fn ax_open_dir(path: &str, opts: &AxOpenOptions) -> AxResult<AxDirHandle> {
    Ok(AxDirHandle(Directory::open_dir(path, opts).await?))
}

pub async fn ax_read_file(file: &mut AxFileHandle, buf: &mut [u8]) -> AxResult<usize> {
    file.0.read(buf).await
}

pub async fn ax_read_file_at(file: &AxFileHandle, offset: u64, buf: &mut [u8]) -> AxResult<usize> {
    file.0.read_at(offset, buf).await
}

pub async fn ax_write_file(file: &mut AxFileHandle, buf: &[u8]) -> AxResult<usize> {
    file.0.write(buf).await
}

pub async fn ax_write_file_at(file: &AxFileHandle, offset: u64, buf: &[u8]) -> AxResult<usize> {
    file.0.write_at(offset, buf).await
}

pub async fn ax_truncate_file(file: &AxFileHandle, size: u64) -> AxResult {
    file.0.truncate(size).await
}

pub async fn ax_flush_file(file: &AxFileHandle) -> AxResult {
    file.0.flush().await
}

pub async fn ax_seek_file(file: &mut AxFileHandle, pos: AxSeekFrom) -> AxResult<u64> {
    file.0.seek(pos).await
}

pub async fn ax_file_attr(file: &AxFileHandle) -> AxResult<AxFileAttr> {
    file.0.get_attr().await
}

pub async fn ax_read_dir(dir: &mut AxDirHandle, dirents: &mut [AxDirEntry]) -> AxResult<usize> {
    dir.0.read_dir(dirents).await
}

pub async fn ax_create_dir(path: &str) -> AxResult {
    async_fs::api::create_dir(path).await
}

pub async fn ax_remove_dir(path: &str) -> AxResult {
    async_fs::api::remove_dir(path).await
}

pub async fn ax_remove_file(path: &str) -> AxResult {
    async_fs::api::remove_file(path).await
}

pub async fn ax_rename(old: &str, new: &str) -> AxResult {
    async_fs::api::rename(old, new).await
}

pub async fn ax_current_dir() -> AxResult<String> {
    async_fs::api::current_dir().await
}

pub async fn ax_set_current_dir(path: &str) -> AxResult {
    async_fs::api::set_current_dir(path).await
}

use core::{pin::Pin, task::{Context, Poll}};

// impl AsyncRead for AxDirHandle {
//     fn read(
//         mut self: Pin<&mut Self>,
//         cx: &mut Context<'_>,
//         buf: &mut [u8],
//     ) -> Poll<AxResult<usize>> {
//         AsyncRead::read(Pin::new(&mut self.0), cx, buf)
//     }
// }

// impl AsyncWrite for AxDirHandle {
//     fn write(
//         mut self: Pin<&mut Self>,
//         cx: &mut Context<'_>,
//         buf: &[u8],
//     ) -> Poll<AxResult<usize>> {
//         AsyncWrite::write(Pin::new(&mut self.0), cx, buf)
//     }

//     fn flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<AxResult<()>> {
//         AsyncWrite::flush(Pin::new(&mut self.0), cx)
//     }

//     fn close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<AxResult<()>> {
//         AsyncWrite::close(Pin::new(&mut self.0), cx)
//     }
// }

// impl AsyncSeek for AxDirHandle {
//     fn seek(
//         mut self: Pin<&mut Self>,
//         cx: &mut Context<'_>,
//         pos: AxSeekFrom,
//     ) -> Poll<AxResult<u64>> {
//         AsyncSeek::seek(Pin::new(&mut self.0), cx, pos)
//     }
// }

impl AxDirHandle {

    pub fn poll_read_dir(
        self: Pin<&mut Self>, 
        cx: &mut Context<'_>, 
        dirents: &mut [AxDirEntry]
    ) -> Poll<AxResult<usize>> {
        Pin::new(&mut self.get_mut().0).poll_read_dir(cx, dirents)
    }
}

impl AsyncRead for AxFileHandle {
    fn read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<AxResult<usize>> {
        AsyncRead::read(Pin::new(&mut self.get_mut().0), cx, buf)
    }
}

impl AsyncWrite for AxFileHandle {
    fn write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<AxResult<usize>> {
        AsyncWrite::write(Pin::new(&mut self.get_mut().0), cx, buf)
    }

    fn flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<AxResult<()>> {
        AsyncWrite::flush(Pin::new(&mut self.get_mut().0), cx)
    }

    fn close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<AxResult<()>> {
        AsyncWrite::close(Pin::new(&mut self.get_mut().0), cx)
    }
}

impl AsyncSeek for AxFileHandle {
    fn seek(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        pos: AxSeekFrom,
    ) -> Poll<AxResult<u64>> {
        AsyncSeek::seek(Pin::new(&mut self.get_mut().0), cx, pos)
    }
}