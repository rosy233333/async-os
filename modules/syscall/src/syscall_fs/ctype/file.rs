extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use async_fs::api::{File, FileIO, FileIOType, Kstat, OpenFlags, SeekFrom};
use async_io::{AsyncRead, AsyncSeek, AsyncWrite};
use axerrno::AxResult;
use core::{
    future::Future,
    pin::Pin,
    task::{ready, Context, Poll},
};

use axlog::debug;

use crate::{new_file, normal_file_mode, StMode, TimeSecs};
use executor::link::get_link_count;
use sync::Mutex;

pub static INODE_NAME_MAP: Mutex<BTreeMap<String, u64>> = Mutex::new(BTreeMap::new());

/// 文件描述符
pub struct FileDesc {
    /// 文件路径
    pub path: String,
    /// 文件
    pub file: Arc<Mutex<File>>,
    /// 文件打开的标志位
    pub flags: Mutex<OpenFlags>,
    /// 文件信息
    pub stat: Mutex<FileMetaData>,
}

/// 文件在os中运行时的可变信息
/// TODO: 暂时全部记为usize
pub struct FileMetaData {
    /// 最后一次访问时间
    pub atime: TimeSecs,
    /// 最后一次改变(modify)内容的时间
    pub mtime: TimeSecs,
    /// 最后一次改变(change)属性的时间
    pub ctime: TimeSecs,
    // /// 打开时的选项。
    // /// 主要用于判断 CLOEXEC，即 exec 时是否关闭。默认为 false。
    // pub flags: OpenFlags,
}

/// 为FileDesc实现 FileIO trait
impl FileIO for FileDesc {
    fn read(self: Pin<&Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<AxResult<usize>> {
        let mut file = ready!(Pin::new(&mut self.file.lock()).poll(cx));
        AsyncRead::read(Pin::new(&mut *file), cx, buf)
    }

    fn write(self: Pin<&Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<AxResult<usize>> {
        let mut file = ready!(Pin::new(&mut self.file.lock()).poll(cx));
        let old_offset = ready!(AsyncSeek::seek(
            Pin::new(&mut *file),
            cx,
            SeekFrom::Current(0)
        ))
        .unwrap();
        // 这里使用了 Box，是否存在不需要进行额外堆分配的操作
        let size = ready!(Box::pin(file.metadata()).as_mut().poll(cx))
            .unwrap()
            .size();
        if old_offset > size {
            let _ = ready!(AsyncSeek::seek(
                Pin::new(&mut *file),
                cx,
                SeekFrom::Start(size)
            ));
            let temp_buf: Vec<u8> = vec![0u8; (old_offset - size) as usize];
            let _ = ready!(AsyncWrite::write(Pin::new(&mut *file), cx, &temp_buf));
        }
        AsyncWrite::write(Pin::new(&mut *file), cx, buf)
    }

    fn flush(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<AxResult<()>> {
        let mut file = ready!(Pin::new(&mut self.file.lock()).poll(cx));
        AsyncWrite::flush(Pin::new(&mut *file), cx)
    }

    fn seek(self: Pin<&Self>, cx: &mut Context<'_>, pos: SeekFrom) -> Poll<AxResult<u64>> {
        let mut file = ready!(Pin::new(&mut self.file.lock()).poll(cx));
        AsyncSeek::seek(Pin::new(&mut *file), cx, pos)
    }

    fn readable(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<bool> {
        let file = ready!(Pin::new(&mut self.file.lock()).poll(cx));
        Poll::Ready(file.readable())
    }

    fn writable(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<bool> {
        let file = ready!(Pin::new(&mut self.file.lock()).poll(cx));
        Poll::Ready(file.writable())
    }

    fn executable(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<bool> {
        let file = ready!(Pin::new(&mut self.file.lock()).poll(cx));
        Poll::Ready(file.executable())
    }

    fn get_type(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<FileIOType> {
        Poll::Ready(FileIOType::FileDesc)
    }

    fn get_path(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<String> {
        Poll::Ready(self.path.clone())
    }

    fn truncate(self: Pin<&Self>, cx: &mut Context<'_>, len: usize) -> Poll<AxResult<()>> {
        let file = ready!(Pin::new(&mut self.file.lock()).poll(cx));
        let res = Box::pin(file.truncate(len as _)).as_mut().poll(cx);
        res
    }

    fn get_stat(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<AxResult<Kstat>> {
        let fut = async {
            let file = self.file.lock().await;
            let attr = file.get_attr().await?;
            let stat = self.stat.lock().await;
            let inode_map = INODE_NAME_MAP.lock().await;
            let inode_number = if let Some(inode_number) = inode_map.get(&self.path) {
                *inode_number
            } else {
                // return Err(axerrno::AxError::NotFound);
                // Now the file exists but it wasn't opened
                drop(inode_map);
                new_inode(self.path.clone()).await?;
                let inode_map = INODE_NAME_MAP.lock().await;
                assert!(inode_map.contains_key(&self.path));
                let number = *(inode_map.get(&self.path).unwrap());
                drop(inode_map);
                number
            };
            let kstat = Kstat {
                st_dev: 1,
                st_ino: inode_number,
                st_mode: normal_file_mode(StMode::S_IFREG).bits() | 0o777,
                st_nlink: get_link_count(&(self.path.as_str().to_string())).await as _,
                st_uid: 0,
                st_gid: 0,
                st_rdev: 0,
                _pad0: 0,
                st_size: attr.size(),
                st_blksize: async_fs::BLOCK_SIZE as u32,
                _pad1: 0,
                st_blocks: attr.blocks(),
                st_atime_sec: stat.atime.tv_sec as isize,
                st_atime_nsec: stat.atime.tv_nsec as isize,
                st_mtime_sec: stat.mtime.tv_sec as isize,
                st_mtime_nsec: stat.mtime.tv_nsec as isize,
                st_ctime_sec: stat.ctime.tv_sec as isize,
                st_ctime_nsec: stat.ctime.tv_nsec as isize,
            };
            Ok(kstat)
        };
        let res = Box::pin(fut).as_mut().poll(cx);
        res
    }

    fn set_status(self: Pin<&Self>, cx: &mut Context<'_>, flags: OpenFlags) -> Poll<bool> {
        *ready!(Pin::new(&mut self.flags.lock()).poll(cx)) = flags;
        Poll::Ready(true)
    }

    fn get_status(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<OpenFlags> {
        Pin::new(&mut self.flags.lock())
            .poll(cx)
            .map(|flags| *flags)
    }

    fn set_close_on_exec(self: Pin<&Self>, cx: &mut Context<'_>, is_set: bool) -> Poll<bool> {
        let mut flags = ready!(Pin::new(&mut self.flags.lock()).poll(cx));
        if is_set {
            // 设置close_on_exec位置
            *flags |= OpenFlags::CLOEXEC;
        } else {
            *flags &= !OpenFlags::CLOEXEC;
        }
        Poll::Ready(true)
    }

    fn ready_to_read(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<bool> {
        if ready!(self.readable(cx)) {
            return Poll::Ready(false);
        }
        // 获取当前的位置
        let now_pos = ready!(self.seek(cx, SeekFrom::Current(0))).unwrap();
        // 获取最后的位置
        let len: u64 = ready!(self.seek(cx, SeekFrom::End(0))).unwrap();
        // 把文件指针复原，因为获取len的时候指向了尾部
        ready!(self.seek(cx, SeekFrom::Start(now_pos))).unwrap();
        Poll::Ready(now_pos != len)
    }

    fn ready_to_write(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<bool> {
        if ready!(self.writable(cx)) {
            return Poll::Ready(false);
        }
        // 获取当前的位置
        let now_pos = ready!(self.seek(cx, SeekFrom::Current(0))).unwrap();
        // 获取最后的位置
        let len: u64 = ready!(self.seek(cx, SeekFrom::End(0))).unwrap();
        // 把文件指针复原，因为获取len的时候指向了尾部
        ready!(self.seek(cx, SeekFrom::Start(now_pos))).unwrap();
        Poll::Ready(now_pos != len)
    }
}

impl FileDesc {
    /// debug

    /// 创建一个新的文件描述符
    pub fn new(path: &str, file: Arc<Mutex<File>>, flags: OpenFlags) -> Self {
        Self {
            path: path.to_string(),
            file,
            flags: Mutex::new(flags),
            stat: Mutex::new(FileMetaData {
                atime: TimeSecs::default(),
                mtime: TimeSecs::default(),
                ctime: TimeSecs::default(),
            }),
        }
    }
}

/// 新建一个文件描述符
pub async fn new_fd(path: String, flags: OpenFlags) -> AxResult<FileDesc> {
    debug!("Into function new_fd, path: {}", path);
    let file = new_file(path.as_str(), &flags).await?;
    // let file_size = file.metadata()?.len();

    let fd = FileDesc::new(path.as_str(), Arc::new(Mutex::new(file)), flags);
    Ok(fd)
}

/// 当新建一个文件或者目录节点时，需要为其分配一个新的inode号
/// 由于我们不涉及删除文件，因此我们可以简单地使用一个全局增的计数器来分配inode号
pub async fn new_inode(path: String) -> AxResult<()> {
    let mut inode_name_map = INODE_NAME_MAP.lock().await;
    if inode_name_map.contains_key(&path) {
        return Ok(());
    }
    let inode_number = inode_name_map.len() as u64 + 1;
    inode_name_map.insert(path, inode_number);
    Ok(())
}
