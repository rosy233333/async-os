use crate::{normal_file_mode, StMode};
extern crate alloc;
use alloc::string::String;
use async_fs::api::{self, FileIO, FileIOType, Kstat, OpenFlags, SeekFrom};
use axerrno::{AxError, AxResult};
use core::{
    pin::Pin,
    task::{Context, Poll},
};

/// 目录描述符
pub struct DirDesc {
    /// 目录
    pub dir_path: String,
}

/// 目录描述符的实现
impl DirDesc {
    /// 创建一个新的目录描述符
    pub fn new(path: String) -> Self {
        Self { dir_path: path }
    }
}

/// 为DirDesc实现FileIO trait
impl FileIO for DirDesc {
    fn read(self: Pin<&Self>, _cx: &mut Context<'_>, _buf: &mut [u8]) -> Poll<AxResult<usize>> {
        Poll::Ready(Err(AxError::IsADirectory))
    }

    fn write(self: Pin<&Self>, _cx: &mut Context<'_>, _buf: &[u8]) -> Poll<AxResult<usize>> {
        Poll::Ready(Err(AxError::IsADirectory))
    }

    fn flush(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<AxResult<()>> {
        Poll::Ready(Err(AxError::IsADirectory))
    }

    fn seek(self: Pin<&Self>, _cx: &mut Context<'_>, _pos: SeekFrom) -> Poll<AxResult<u64>> {
        Poll::Ready(Err(AxError::IsADirectory))
    }

    fn get_type(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<FileIOType> {
        Poll::Ready(FileIOType::DirDesc)
    }

    fn executable(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<bool> {
        Poll::Ready(false)
    }

    fn readable(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<bool> {
        Poll::Ready(false)
    }

    fn writable(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<bool> {
        Poll::Ready(false)
    }

    fn get_path(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<String> {
        Poll::Ready(self.dir_path.clone())
    }

    fn get_stat(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<AxResult<Kstat>> {
        let kstat = Kstat {
            st_dev: 1,
            st_ino: 0,
            st_mode: normal_file_mode(StMode::S_IFDIR).bits(),
            st_nlink: 1,
            st_uid: 0,
            st_gid: 0,
            st_rdev: 0,
            _pad0: 0,
            st_size: 0,
            st_blksize: 0,
            _pad1: 0,
            st_blocks: 0,
            st_atime_sec: 0,
            st_atime_nsec: 0,
            st_mtime_sec: 0,
            st_mtime_nsec: 0,
            st_ctime_sec: 0,
            st_ctime_nsec: 0,
        };
        Poll::Ready(Ok(kstat))
    }

    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
}

pub async fn new_dir(dir_path: String, _flags: OpenFlags) -> AxResult<DirDesc> {
    debug!("Into function new_dir, dir_path: {}", dir_path);
    if !api::path_exists(dir_path.as_str()).await {
        // api::create_dir_all(dir_path.as_str())?;
        api::create_dir(dir_path.as_str()).await?;
    }
    Ok(DirDesc::new(dir_path))
}
