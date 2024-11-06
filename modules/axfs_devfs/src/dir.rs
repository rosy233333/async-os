use alloc::collections::BTreeMap;
use alloc::sync::{Arc, Weak};
use async_vfs::{VfsDirEntry, VfsNodeAttr, VfsNodeOps, VfsNodeRef, VfsNodeType};
use async_vfs::{VfsError, VfsResult};
use core::{
    pin::Pin,
    task::{Context, Poll},
};
use spin::RwLock;

/// The directory node in the device filesystem.
///
/// It implements [`axfs_vfs::VfsNodeOps`].
pub struct DirNode {
    inner: Arc<DirNodeInner>,
}

pub struct DirNodeInner {
    parent: RwLock<Weak<dyn VfsNodeOps + Unpin + Send + Sync>>,
    children: RwLock<BTreeMap<&'static str, VfsNodeRef>>,
}

impl DirNode {
    pub(super) fn new(parent: Option<&VfsNodeRef>) -> Arc<Self> {
        let parent = parent.map_or(Weak::<Self>::new() as _, Arc::downgrade);
        Arc::new(Self {
            inner: Arc::new(DirNodeInner {
                parent: RwLock::new(parent),
                children: RwLock::new(BTreeMap::new()),
            }),
        })
    }

    pub(super) fn set_parent(&self, parent: Option<&VfsNodeRef>) {
        *self.inner.parent.write() = parent.map_or(Weak::<Self>::new() as _, Arc::downgrade);
    }

    /// Create a subdirectory at this directory.
    pub fn mkdir(self: &Arc<Self>, name: &'static str) -> Arc<Self> {
        let parent = self.clone() as VfsNodeRef;
        let node = Self::new(Some(&parent));
        self.inner.children.write().insert(name, node.clone());
        node
    }

    /// Add a node to this directory.
    pub fn add(&self, name: &'static str, node: VfsNodeRef) {
        self.inner.children.write().insert(name, node);
    }
}

impl VfsNodeOps for DirNode {
    fn poll_get_attr(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<VfsResult<VfsNodeAttr>> {
        Poll::Ready(Ok(VfsNodeAttr::new_dir(4096, 0)))
    }

    fn poll_parent(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<Option<VfsNodeRef>> {
        Poll::Ready(self.inner.parent.read().upgrade())
    }

    fn poll_lookup(
        self: Pin<&Self>,
        cx: &mut Context<'_>,
        path: &str,
    ) -> Poll<VfsResult<VfsNodeRef>> {
        let (name, rest) = split_path(path);
        let node = core::task::ready!(match name {
            "" | "." => Poll::Ready(Ok(Arc::new(Self {
                inner: self.inner.clone()
            }) as VfsNodeRef)),
            ".." => self
                .poll_parent(cx)
                .map(|inner| inner.ok_or(VfsError::NotFound)),
            _ => Poll::Ready(
                self.inner
                    .children
                    .read()
                    .get(name)
                    .cloned()
                    .ok_or(VfsError::NotFound)
            ),
        }?);

        if let Some(rest) = rest {
            VfsNodeOps::poll_lookup(Pin::new(&node), cx, rest)
        } else {
            Poll::Ready(Ok(node))
        }
    }

    fn poll_read_dir(
        self: Pin<&Self>,
        cx: &mut Context<'_>,
        start_idx: usize,
        dirents: &mut [VfsDirEntry],
    ) -> Poll<VfsResult<usize>> {
        let children = self.inner.children.read();
        let mut children = children.iter().skip(start_idx.max(2) - 2);
        for (i, ent) in dirents.iter_mut().enumerate() {
            match i + start_idx {
                0 => *ent = VfsDirEntry::new(".", VfsNodeType::Dir),
                1 => *ent = VfsDirEntry::new("..", VfsNodeType::Dir),
                _ => {
                    if let Some((name, node)) = children.next() {
                        *ent = VfsDirEntry::new(
                            name,
                            core::task::ready!(VfsNodeOps::poll_get_attr(Pin::new(node), cx))
                                .unwrap()
                                .file_type(),
                        );
                    } else {
                        return Poll::Ready(Ok(i));
                    }
                }
            }
        }
        Poll::Ready(Ok(dirents.len()))
    }

    fn poll_create(
        self: Pin<&Self>,
        cx: &mut Context<'_>,
        path: &str,
        ty: VfsNodeType,
    ) -> Poll<VfsResult> {
        log::debug!("create {:?} at devfs: {}", ty, path);
        let (name, rest) = split_path(path);
        if let Some(rest) = rest {
            match name {
                "" | "." => self.poll_create(cx, rest, ty),
                ".." => {
                    let node = core::task::ready!(self
                        .poll_parent(cx)
                        .map(|inner| inner.ok_or(VfsError::NotFound)))?;
                    VfsNodeOps::poll_create(Pin::new(&node), cx, rest, ty)
                }
                _ => {
                    let children = self.inner.children.read();
                    let node = children.get(name).ok_or(VfsError::NotFound)?;
                    VfsNodeOps::poll_create(Pin::new(node), cx, rest, ty)
                }
            }
        } else if name.is_empty() || name == "." || name == ".." {
            Poll::Ready(Ok(())) // already exists
        } else {
            Poll::Ready(Err(VfsError::PermissionDenied)) // do not support to create nodes dynamically
        }
    }

    fn poll_remove(self: Pin<&Self>, cx: &mut Context<'_>, path: &str) -> Poll<VfsResult> {
        log::debug!("remove at devfs: {}", path);
        let (name, rest) = split_path(path);
        if let Some(rest) = rest {
            match name {
                "" | "." => self.poll_remove(cx, rest),
                ".." => {
                    let node = core::task::ready!(self
                        .poll_parent(cx)
                        .map(|inner| inner.ok_or(VfsError::NotFound)))?;
                    VfsNodeOps::poll_remove(Pin::new(&node), cx, rest)
                }
                _ => {
                    let children = self.inner.children.read();
                    let node = children.get(name).ok_or(VfsError::NotFound)?;
                    VfsNodeOps::poll_remove(Pin::new(node), cx, rest)
                }
            }
        } else {
            Poll::Ready(Err(VfsError::PermissionDenied)) // do not support to remove nodes dynamically
        }
    }

    async_vfs::impl_vfs_dir_default! {}
}

fn split_path(path: &str) -> (&str, Option<&str>) {
    let trimmed_path = path.trim_start_matches('/');
    trimmed_path.find('/').map_or((trimmed_path, None), |n| {
        (&trimmed_path[..n], Some(&trimmed_path[n + 1..]))
    })
}
