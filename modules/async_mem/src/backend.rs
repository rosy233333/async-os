use alloc::boxed::Box;
use async_fs::api::{AsAny, File, FileExt};
use async_io::{AsyncRead, AsyncSeek, Seek, SeekFrom};
use core::{
    pin::Pin,
    task::{Context, Poll},
};

pub type BackEndFile = FileExt;

/// File backend for Lazy load `MapArea`. `file` should be a file holding a offset value. Normally,
/// `MemBackend` won't share a file with other things, so we use a `Box` here.
pub struct MemBackend {
    file: BackEndFile,
}

impl MemBackend {
    /// Create a new `MemBackend` with a file and the seek offset of this file.
    pub async fn new(mut file: BackEndFile, offset: u64) -> Self {
        let _ = file.seek(SeekFrom::Start(offset)).await.unwrap();

        Self { file }
    }

    /// clone a new `MemBackend` with a delta offset of the file of the original `MemBackend`.
    pub async fn clone_with_delta(&self, delta: i64) -> Self {
        let mut new_backend = self.clone();

        let _ = new_backend.seek(SeekFrom::Current(delta)).await.unwrap();

        new_backend
    }

    /// read from the file of the `MemBackend` with a pos offset.
    pub async fn read_from_seek(
        &mut self,
        pos: SeekFrom,
        buf: &mut [u8],
    ) -> Result<usize, async_io::Error> {
        self.file.read_from_seek(pos, buf).await
    }

    /// write to the file of the `MemBackend` with a pos offset.
    pub async fn write_to_seek(
        &mut self,
        pos: SeekFrom,
        buf: &[u8],
    ) -> Result<usize, async_io::Error> {
        self.file.write_to_seek(pos, buf).await
    }

    /// whether the file of the `MemBackend` is readable.
    pub async fn readable(&self) -> bool {
        self.file.readable().await
    }

    /// whether the file of the `MemBackend` is writable.
    pub async fn writable(&self) -> bool {
        self.file.writable().await
    }
}

impl Clone for MemBackend {
    fn clone(&self) -> Self {
        let file = self
            .file
            .inner
            .as_any()
            .downcast_ref::<File>()
            .expect("Cloning a MemBackend with a non-file object")
            .clone();

        Self {
            file: FileExt {
                inner: Box::new(file),
            },
        }
    }
}

impl AsyncSeek for MemBackend {
    fn seek(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        pos: SeekFrom,
    ) -> Poll<async_io::Result<u64>> {
        Pin::new(&mut *self.file.inner).seek(cx, pos)
    }
}

impl AsyncRead for MemBackend {
    fn read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<async_io::Result<usize>> {
        Pin::new(&mut *self.file.inner).read(cx, buf)
    }
}
