/// When implement [`VfsNodeOps`] on a directory node, add dummy file operations
/// that just return an error.
///
/// [`VfsNodeOps`]: crate::VfsNodeOps
#[macro_export]
macro_rules! impl_vfs_dir_default {
    () => {
        fn read_at(
            self: core::pin::Pin<&Self>,
            _cx: &mut core::task::Context<'_>,
            _offset: u64,
            _buf: &mut [u8],
        ) -> core::task::Poll<$crate::VfsResult<usize>> {
            core::task::Poll::Ready($crate::__priv::ax_err!(IsADirectory))
        }

        fn write_at(
            self: core::pin::Pin<&Self>,
            _cx: &mut core::task::Context<'_>,
            _offset: u64,
            _buf: &[u8],
        ) -> core::task::Poll<$crate::VfsResult<usize>> {
            core::task::Poll::Ready($crate::__priv::ax_err!(IsADirectory))
        }

        fn fsync(
            self: core::pin::Pin<&Self>,
            _cx: &mut core::task::Context<'_>,
        ) -> core::task::Poll<$crate::VfsResult> {
            core::task::Poll::Ready($crate::__priv::ax_err!(IsADirectory))
        }

        fn truncate(
            self: core::pin::Pin<&Self>,
            _cx: &mut core::task::Context<'_>,
            _size: u64,
        ) -> core::task::Poll<$crate::VfsResult> {
            core::task::Poll::Ready($crate::__priv::ax_err!(IsADirectory))
        }

        #[inline]
        fn as_any(&self) -> &dyn core::any::Any {
            self
        }
    };
}

/// When implement [`VfsNodeOps`] on a non-directory node, add dummy directory
/// operations that just return an error.
///
/// [`VfsNodeOps`]: crate::VfsNodeOps
#[macro_export]
macro_rules! impl_vfs_non_dir_default {
    () => {
        fn lookup(
            self: core::pin::Pin<&Self>,
            _cx: &mut core::task::Context<'_>,
            _path: &str,
        ) -> core::task::Poll<$crate::VfsResult<$crate::VfsNodeRef>> {
            core::task::Poll::Ready($crate::__priv::ax_err!(NotADirectory))
        }

        fn create(
            self: core::pin::Pin<&Self>,
            _cx: &mut core::task::Context<'_>,
            _path: &str,
            _ty: $crate::VfsNodeType,
        ) -> core::task::Poll<$crate::VfsResult> {
            core::task::Poll::Ready($crate::__priv::ax_err!(NotADirectory))
        }

        fn remove(
            self: core::pin::Pin<&Self>,
            _cx: &mut core::task::Context<'_>,
            _path: &str,
        ) -> core::task::Poll<$crate::VfsResult> {
            core::task::Poll::Ready($crate::__priv::ax_err!(NotADirectory))
        }

        fn read_dir(
            self: core::pin::Pin<&Self>,
            _cx: &mut core::task::Context<'_>,
            _start_idx: usize,
            _dirents: &mut [VfsDirEntry],
        ) -> core::task::Poll<$crate::VfsResult<usize>> {
            core::task::Poll::Ready($crate::__priv::ax_err!(NotADirectory))
        }

        #[inline]
        fn as_any(&self) -> &dyn core::any::Any {
            self
        }
    };
}
