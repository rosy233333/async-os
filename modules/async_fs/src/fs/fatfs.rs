
use alloc::sync::Arc;
use core::cell::UnsafeCell;
use core::future::Future;

use async_vfs::{VfsDirEntry, VfsError, VfsNodePerm, VfsResult};
use async_vfs::{VfsNodeAttr, VfsNodeOps, VfsNodeRef, VfsNodeType, VfsOps};
use sync::Mutex;
use fatfs::{Dir, File, LossyOemCpConverter, NullTimeProvider, Read, Seek, SeekFrom, Write};
use core::{pin::Pin, task::{Context, Poll}};

use crate::dev::Disk;

pub const BLOCK_SIZE: usize = 512;

pub struct FatFileSystem {
    inner: fatfs::FileSystem<Disk, NullTimeProvider, LossyOemCpConverter>,
    root_dir: UnsafeCell<Option<VfsNodeRef>>,
}

pub struct FileWrapper<'a>(Mutex<File<'a, Disk, NullTimeProvider, LossyOemCpConverter>>);
pub struct DirWrapper<'a>(Dir<'a, Disk, NullTimeProvider, LossyOemCpConverter>);

unsafe impl Sync for FatFileSystem {}
unsafe impl Send for FatFileSystem {}
unsafe impl<'a> Send for FileWrapper<'a> {}
unsafe impl<'a> Sync for FileWrapper<'a> {}
unsafe impl<'a> Send for DirWrapper<'a> {}
unsafe impl<'a> Sync for DirWrapper<'a> {}

impl FatFileSystem {
    #[cfg(feature = "use-ramdisk")]
    pub fn new(mut disk: Disk) -> Self {
        let opts = fatfs::FormatVolumeOptions::new();
        fatfs::format_volume(&mut disk, opts).expect("failed to format volume");
        let inner = fatfs::FileSystem::new(disk, fatfs::FsOptions::new())
            .expect("failed to initialize FAT filesystem");
        Self {
            inner,
            root_dir: UnsafeCell::new(None),
        }
    }

    #[cfg(not(feature = "use-ramdisk"))]
    pub fn new(disk: Disk) -> Self {
        let inner = fatfs::FileSystem::new(disk, fatfs::FsOptions::new())
            .expect("failed to initialize FAT filesystem");
        Self {
            inner,
            root_dir: UnsafeCell::new(None),
        }
    }

    pub fn init(&'static self) {
        // must be called before later operations
        unsafe { *self.root_dir.get() = Some(Self::new_dir(self.inner.root_dir())) }
    }

    fn new_file(file: File<'_, Disk, NullTimeProvider, LossyOemCpConverter>) -> Arc<FileWrapper> {
        Arc::new(FileWrapper(Mutex::new(file)))
    }

    fn new_dir(dir: Dir<'_, Disk, NullTimeProvider, LossyOemCpConverter>) -> Arc<DirWrapper> {
        Arc::new(DirWrapper(dir))
    }
}

impl VfsNodeOps for FileWrapper<'static> {
    async_vfs::impl_vfs_non_dir_default! {}

    fn get_attr(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<VfsResult<VfsNodeAttr>> {
        Pin::new(&mut self.0.lock()).poll(cx).map(|mut file| {
            let size = file.seek(SeekFrom::End(0)).map_err(as_vfs_err)?;
            let blocks = (size + BLOCK_SIZE as u64 - 1) / BLOCK_SIZE as u64;
            // FAT fs doesn't support permissions, we just set everything to 755
            let perm = VfsNodePerm::from_bits_truncate(0o755);
            Ok(VfsNodeAttr::new(perm, VfsNodeType::File, size, blocks))
        })
    }

    fn read_at(self: Pin<&Self>, cx: &mut Context<'_>, offset: u64, buf: &mut [u8]) -> Poll<VfsResult<usize>> {
        Pin::new(&mut self.0.lock()).poll(cx).map(|mut file| {
            file.seek(SeekFrom::Start(offset)).map_err(as_vfs_err)?; // TODO: more efficient
            let buf_len = buf.len();
            let mut now_offset = 0;
            let mut probe = buf.to_vec();
            while now_offset < buf_len {
                let ans = file.read(&mut probe).map_err(as_vfs_err);
                if ans.is_err() {
                    return ans;
                }
                let read_len = ans.unwrap();

                if read_len == 0 {
                    break;
                }
                buf[now_offset..now_offset + read_len].copy_from_slice(&probe[..read_len]);
                now_offset += read_len;
                probe = probe[read_len..].to_vec();
            }
            Ok(now_offset)
        })
    }

    fn write_at(
        self: Pin<&Self>, 
        cx: &mut Context<'_>, 
        offset: u64, 
        buf: &[u8]
    ) -> Poll<VfsResult<usize>> {
        Pin::new(&mut self.0.lock()).poll(cx).map(|mut file| {
            file.seek(SeekFrom::Start(offset)).map_err(as_vfs_err)?; // TODO: more efficient
            let buf_len = buf.len();
            let mut now_offset = 0;
            let mut probe = buf.to_vec();
            while now_offset < buf_len {
                let ans = file.write(&probe).map_err(as_vfs_err);
                if ans.is_err() {
                    return ans;
                }
                let write_len = ans.unwrap();

                if write_len == 0 {
                    break;
                }
                now_offset += write_len;
                probe = probe[write_len..].to_vec();
            }
            Ok(now_offset)
        })
    }

    fn truncate(self: Pin<&Self>, cx: &mut Context<'_>, size: u64) -> Poll<VfsResult> {
        Pin::new(&mut self.0.lock()).poll(cx).map(|mut file| {
            file.seek(SeekFrom::Start(size)).map_err(as_vfs_err)?; // TODO: more efficient
            file.truncate().map_err(as_vfs_err)
        })
    }
}

impl VfsNodeOps for DirWrapper<'static> {
    async_vfs::impl_vfs_dir_default! {}

    fn get_attr(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<VfsResult<VfsNodeAttr>> {
        Poll::Ready(Ok(VfsNodeAttr::new(
            VfsNodePerm::from_bits_truncate(0o755),
            VfsNodeType::Dir,
            BLOCK_SIZE as u64,
            1,
        )))
    }

    fn parent(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<Option<VfsNodeRef>> {
        Poll::Ready(self.0
            .open_dir("..")
            .map_or(None, |dir| Some(FatFileSystem::new_dir(dir)))
        )
    }

    fn lookup(self: Pin<&Self>, cx: &mut Context<'_>, path: &str) -> Poll<VfsResult<VfsNodeRef>> {
        debug!("lookup at fatfs: {}", path);
        let path = path.trim_matches('/');
        if path.is_empty() || path == "." {
            let dir = self.0.clone();
            let dir_wrapper = FatFileSystem::new_dir(dir);
            return Poll::Ready(Ok(dir_wrapper));
        }

        if let Some(rest) = path.strip_prefix("./") {
            return self.lookup(cx, rest);
        }

        // TODO: use `fatfs::Dir::find_entry`, but it's not public.
        if let Some((dir, rest)) = path.split_once('/') {
            let dir = futures_core::ready!(self.lookup(cx, dir))?;
            return VfsNodeOps::lookup(Pin::new(&dir), cx, rest);
        }

        for entry in self.0.iter() {
            let Ok(entry) = entry else {
                return Poll::Ready(Err(VfsError::Io));
            };

            if entry.file_name() == path {
                if entry.is_file() {
                    return Poll::Ready(Ok(FatFileSystem::new_file(entry.to_file())));
                } else if entry.is_dir() {
                    return Poll::Ready(Ok(FatFileSystem::new_dir(entry.to_dir())));
                }
            }
        }
        Poll::Ready(Err(VfsError::NotFound))
    }

    fn create(self: Pin<&Self>, cx: &mut Context<'_>, path: &str, ty: VfsNodeType) -> Poll<VfsResult> {
        debug!("create {:?} at fatfs: {}", ty, path);
        let path = path.trim_matches('/');
        if path.is_empty() || path == "." {
            return Poll::Ready(Ok(()));
        }
        if let Some(rest) = path.strip_prefix("./") {
            return self.create(cx, rest, ty);
        }
        match ty {
            VfsNodeType::File => {
                self.0.create_file(path).map_err(as_vfs_err)?;
                Poll::Ready(Ok(()))
            }
            VfsNodeType::Dir => {
                self.0.create_dir(path).map_err(as_vfs_err)?;
                Poll::Ready(Ok(()))
            }
            _ => Poll::Ready(Err(VfsError::Unsupported)),
        }
    }

    fn remove(self: Pin<&Self>, cx: &mut Context<'_>, path: &str) -> Poll<VfsResult> {
        debug!("remove at fatfs: {}", path);
        let path = path.trim_matches('/');
        assert!(!path.is_empty()); // already check at `root.rs`
        if let Some(rest) = path.strip_prefix("./") {
            return self.remove(cx, rest);
        }
        Poll::Ready(self.0.remove(path).map_err(as_vfs_err))
    }

    fn read_dir(
        self: Pin<&Self>, 
        _cx: &mut Context<'_>, 
        start_idx: usize, 
        dirents: &mut [VfsDirEntry]
    ) -> Poll<VfsResult<usize>> {
        let mut iter = self.0.iter().skip(start_idx);
        for (i, out_entry) in dirents.iter_mut().enumerate() {
            let x = iter.next();
            match x {
                Some(Ok(entry)) => {
                    let ty = if entry.is_dir() {
                        VfsNodeType::Dir
                    } else if entry.is_file() {
                        VfsNodeType::File
                    } else {
                        unreachable!()
                    };
                    *out_entry = VfsDirEntry::new(&entry.file_name(), ty);
                }
                _ => return Poll::Ready(Ok(i)),
            }
        }
        Poll::Ready(Ok(dirents.len()))
    }

    fn rename(
        self: Pin<&Self>, 
        _cx: &mut Context<'_>, 
        src_path: &str, 
        dst_path: &str
    ) -> Poll<VfsResult> {
        // `src_path` and `dst_path` should in the same mounted fs
        debug!(
            "rename at fatfs, src_path: {}, dst_path: {}",
            src_path, dst_path
        );
        let dst_path = dst_path.trim_matches('/');
        Poll::Ready(self.0
            .rename(src_path, &self.0, dst_path)
            .map_err(as_vfs_err)
        )
    }

}

impl VfsOps for FatFileSystem {
    fn root_dir(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<VfsNodeRef> {
        let root_dir = unsafe { (*self.root_dir.get()).as_ref().unwrap() };
        Poll::Ready(root_dir.clone())
    }
}

impl fatfs::IoBase for Disk {
    type Error = ();
}

impl Read for Disk {
    fn read(&mut self, mut buf: &mut [u8]) -> Result<usize, Self::Error> {
        let mut read_len = 0;
        while !buf.is_empty() {
            match self.read_one(buf) {
                Ok(0) => break,
                Ok(n) => {
                    let tmp = buf;
                    buf = &mut tmp[n..];
                    read_len += n;
                }
                Err(_) => return Err(()),
            }
        }
        Ok(read_len)
    }
}

impl Write for Disk {
    fn write(&mut self, mut buf: &[u8]) -> Result<usize, Self::Error> {
        let mut write_len = 0;
        while !buf.is_empty() {
            match self.write_one(buf) {
                Ok(0) => break,
                Ok(n) => {
                    buf = &buf[n..];
                    write_len += n;
                }
                Err(_) => return Err(()),
            }
        }
        Ok(write_len)
    }
    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl Seek for Disk {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, Self::Error> {
        let size = self.size();
        let new_pos = match pos {
            SeekFrom::Start(pos) => Some(pos),
            SeekFrom::Current(off) => self.position().checked_add_signed(off),
            SeekFrom::End(off) => size.checked_add_signed(off),
        }
        .ok_or(())?;
        if new_pos > size {
            warn!("Seek beyond the end of the block device");
        }
        self.set_position(new_pos);
        Ok(new_pos)
    }
}

const fn as_vfs_err(err: fatfs::Error<()>) -> VfsError {
    use fatfs::Error::*;
    match err {
        AlreadyExists => VfsError::AlreadyExists,
        CorruptedFileSystem => VfsError::InvalidData,
        DirectoryIsNotEmpty => VfsError::DirectoryNotEmpty,
        InvalidInput | InvalidFileNameLength | UnsupportedFileNameCharacter => {
            VfsError::InvalidInput
        }
        NotEnoughSpace => VfsError::StorageFull,
        NotFound => VfsError::NotFound,
        UnexpectedEof => VfsError::UnexpectedEof,
        WriteZero => VfsError::WriteZero,
        Io(_) => VfsError::Io,
        _ => VfsError::Io,
    }
}
