use alloc::collections::BTreeMap;
use alloc::sync::{Arc, Weak};
use alloc::{string::String, vec::Vec};

use async_vfs::{VfsDirEntry, VfsNodeAttr, VfsNodeOps, VfsNodeRef, VfsNodeType};
use async_vfs::{VfsError, VfsResult};
use core::{
    pin::Pin,
    task::{Context, Poll},
};
use spin::RwLock;

use crate::file::FileNode;
use crate::Interrupts;

/// The directory node in the RAM filesystem.
///
/// It implements [`axfs_vfs::VfsNodeOps`].
pub struct DirNode {
    this: Weak<DirNode>,
    parent: RwLock<Weak<dyn VfsNodeOps + Unpin + Send + Sync>>,
    children: RwLock<BTreeMap<String, VfsNodeRef>>,
}

impl DirNode {
    pub(super) fn new(parent: Option<Weak<dyn VfsNodeOps + Unpin + Send + Sync>>) -> Arc<Self> {
        Arc::new_cyclic(|this| Self {
            this: this.clone(),
            parent: RwLock::new(parent.unwrap_or_else(|| Weak::<Self>::new())),
            children: RwLock::new(BTreeMap::new()),
        })
    }

    pub(super) fn set_parent(&self, parent: Option<&VfsNodeRef>) {
        *self.parent.write() = parent.map_or(Weak::<Self>::new() as _, Arc::downgrade);
    }

    /// Returns a string list of all entries in this directory.
    pub fn get_entries(&self) -> Vec<String> {
        self.children.read().keys().cloned().collect()
    }

    /// Checks whether a node with the given name exists in this directory.
    pub fn exist(&self, name: &str) -> bool {
        self.children.read().contains_key(name)
    }

    /// Creates a new node with the given name and type in this directory.
    pub fn create_node(&self, name: &str, ty: VfsNodeType) -> VfsResult {
        if self.exist(name) {
            log::error!("AlreadyExists {}", name);
            return Err(VfsError::AlreadyExists);
        }
        let node: VfsNodeRef = match ty {
            VfsNodeType::File => {
                // 当前仅是将interrups作为一个特殊的节点，未来应该进行统一
                if name == "interrupts" {
                    Arc::new(Interrupts)
                } else {
                    Arc::new(FileNode::new())
                }
            }
            VfsNodeType::Dir => Self::new(Some(self.this.clone())),
            _ => return Err(VfsError::Unsupported),
        };
        // let a = node.as_any().downcast_ref::<DirNode>();
        self.children.write().insert(name.into(), node);
        Ok(())
    }

    /// Removes a node by the given name in this directory.
    pub fn remove_node(&self, name: &str) -> VfsResult {
        let mut children = self.children.write();
        let node = children.get(name).ok_or(VfsError::NotFound)?;
        if let Some(dir) = node.as_any().downcast_ref::<DirNode>() {
            if !dir.children.read().is_empty() {
                return Err(VfsError::DirectoryNotEmpty);
            }
        }
        children.remove(name);
        Ok(())
    }
}

impl VfsNodeOps for DirNode {
    fn poll_get_attr(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<VfsResult<VfsNodeAttr>> {
        Poll::Ready(Ok(VfsNodeAttr::new_dir(4096, 0)))
    }

    fn parent(&self) -> Option<VfsNodeRef> {
        self.parent.read().upgrade()
    }

    fn lookup(self: Arc<Self>, path: &str) -> VfsResult<VfsNodeRef> {
        let (name, rest) = split_path(path);
        let node = match name {
            "" | "." => Ok(self.clone() as VfsNodeRef),
            ".." => self.parent().ok_or(VfsError::NotFound),
            _ => self
                    .children
                    .read()
                    .get(name)
                    .cloned()
                    .ok_or(VfsError::NotFound)
            ,
        }?;
        if let Some(rest) = rest {
            node.lookup(rest)
        } else {
            Ok(node)
        }
    }

    fn poll_read_dir(
        self: Pin<&Self>,
        cx: &mut Context<'_>,
        start_idx: usize,
        dirents: &mut [VfsDirEntry],
    ) -> Poll<VfsResult<usize>> {
        let children = self.children.read();
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
        log::debug!("create {:?} at ramfs: {}", ty, path);
        let (name, rest) = split_path(path);
        if let Some(rest) = rest {
            match name {
                "" | "." => self.poll_create(cx, rest, ty),
                ".." => {
                    let node = self.parent().ok_or(VfsError::NotFound)?;
                    VfsNodeOps::poll_create(Pin::new(&node), cx, rest, ty)
                }
                _ => {
                    let children = self.children.read();
                    let node = children.get(name).ok_or(VfsError::NotFound)?;
                    VfsNodeOps::poll_create(Pin::new(node), cx, rest, ty)
                }
            }
        } else if name.is_empty() || name == "." || name == ".." {
            Poll::Ready(Ok(())) // already exists
        } else {
            Poll::Ready(self.create_node(name, ty))
        }
    }

    fn poll_remove(self: Pin<&Self>, cx: &mut Context<'_>, path: &str) -> Poll<VfsResult> {
        log::debug!("remove at ramfs: {}", path);
        let (name, rest) = split_path(path);
        if let Some(rest) = rest {
            match name {
                "" | "." => self.poll_remove(cx, rest),
                ".." => {
                    let node = self.parent().ok_or(VfsError::NotFound)?;
                    VfsNodeOps::poll_remove(Pin::new(&node), cx, rest)
                }
                _ => {
                    let children = self.children.read();
                    let node = children.get(name).ok_or(VfsError::NotFound)?;
                    VfsNodeOps::poll_remove(Pin::new(node), cx, rest)
                }
            }
        } else if name.is_empty() || name == "." || name == ".." {
            Poll::Ready(Err(VfsError::InvalidInput)) // remove '.' or '..
        } else {
            Poll::Ready(self.remove_node(name))
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
