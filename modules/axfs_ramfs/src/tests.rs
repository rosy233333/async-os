use std::sync::Arc;

use async_vfs::{AsyncVfsNodeOps, VfsError, VfsNodeType, VfsResult};

use crate::*;

async fn test_ramfs_ops(devfs: &RamFileSystem) -> VfsResult {
    const N: usize = 32;
    const N_HALF: usize = N / 2;
    let mut buf = [1; N];

    let root = devfs.root_dir();
    assert!(root.get_attr().await?.is_dir());
    assert_eq!(root.get_attr().await?.file_type(), VfsNodeType::Dir);
    assert_eq!(
        root.clone().lookup("urandom").err(),
        Some(VfsError::NotFound)
    );
    assert_eq!(
        root.clone().lookup("f1/").err(),
        Some(VfsError::NotADirectory)
    );

    let node = root.lookup("////f1")?;
    assert_eq!(node.get_attr().await?.file_type(), VfsNodeType::File);
    assert!(!node.get_attr().await?.is_dir());
    assert_eq!(node.get_attr().await?.size(), 0);
    assert_eq!(node.read_at(0, &mut buf).await?, 0);
    assert_eq!(buf, [1; N]);

    assert_eq!(node.write_at(N_HALF as _, &buf[..N_HALF]).await?, N_HALF);
    assert_eq!(node.read_at(0, &mut buf).await?, N);
    assert_eq!(buf[..N_HALF], [0; N_HALF]);
    assert_eq!(buf[N_HALF..], [1; N_HALF]);
    assert_eq!(node.lookup("/").err(), Some(VfsError::NotADirectory));

    let foo = devfs.root_dir().lookup(".///.//././/.////foo")?;
    assert!(foo.get_attr().await?.is_dir());
    assert_eq!(
        foo.read_at(10, &mut buf).await.err(),
        Some(VfsError::IsADirectory)
    );
    assert!(Arc::ptr_eq(
        &foo.clone().lookup("/f3")?,
        &devfs.root_dir().lookup(".//./foo///f3")?,
    ));
    assert_eq!(
        foo.clone()
            .lookup("/bar//f4")?
            .get_attr()
            .await?
            .file_type(),
        VfsNodeType::File
    );
    assert_eq!(
        foo.lookup("/bar///")?.get_attr().await?.file_type(),
        VfsNodeType::Dir
    );

    Ok(())
}

async fn test_get_parent(devfs: &RamFileSystem) -> VfsResult {
    let root = devfs.root_dir();
    assert!(root.parent().is_none());

    let node = root.clone().lookup("f1")?;
    assert!(node.parent().is_none());

    let node = root.clone().lookup(".//foo/bar")?;
    assert!(node.parent().is_some());
    let parent = node.parent().unwrap();
    assert!(Arc::ptr_eq(&parent, &root.clone().lookup("foo")?));
    assert!(parent.lookup("bar").is_ok());

    let node = root.clone().lookup("foo/..")?;
    assert!(Arc::ptr_eq(&node, &root.clone().lookup(".")?));

    assert!(Arc::ptr_eq(
        &root.clone().lookup("/foo/..")?,
        &devfs.root_dir().lookup(".//./foo/././bar/../..")?,
    ));
    assert!(Arc::ptr_eq(
        &root.clone().lookup("././/foo//./../foo//bar///..//././")?,
        &devfs.root_dir().lookup(".//./foo/")?,
    ));
    assert!(Arc::ptr_eq(
        &root.clone().lookup("///foo//bar///../f3")?,
        &root.lookup("foo/.//f3")?,
    ));

    Ok(())
}

async fn test() {
    // .
    // ├── foo
    // │   ├── bar
    // │   │   └── f4
    // │   └── f3
    // ├── f1
    // └── f2

    let ramfs = RamFileSystem::new();
    let root = ramfs.root_dir();
    root.create("f1", VfsNodeType::File).await.unwrap();
    root.create("f2", VfsNodeType::File).await.unwrap();
    root.create("foo", VfsNodeType::Dir).await.unwrap();

    let dir_foo = root.lookup("foo").unwrap();
    dir_foo.create("f3", VfsNodeType::File).await.unwrap();
    dir_foo.create("bar", VfsNodeType::Dir).await.unwrap();

    let dir_bar = dir_foo.lookup("bar").unwrap();
    dir_bar.create("f4", VfsNodeType::File).await.unwrap();

    let mut entries = ramfs.root_dir_node().get_entries();
    entries.sort();
    assert_eq!(entries, ["f1", "f2", "foo"]);

    test_ramfs_ops(&ramfs).await.unwrap();
    test_get_parent(&ramfs).await.unwrap();

    let root = ramfs.root_dir();
    assert_eq!(root.remove("f1").await, Ok(()));
    assert_eq!(root.remove("//f2").await, Ok(()));
    assert_eq!(root.remove("f3").await.err(), Some(VfsError::NotFound));
    assert_eq!(
        root.remove("foo").await.err(),
        Some(VfsError::DirectoryNotEmpty)
    );
    assert_eq!(
        root.remove("foo/..").await.err(),
        Some(VfsError::InvalidInput)
    );
    assert_eq!(
        root.remove("foo/./bar").await.err(),
        Some(VfsError::DirectoryNotEmpty)
    );
    assert_eq!(root.remove("foo/bar/f4").await, Ok(()));
    assert_eq!(root.remove("foo/bar").await, Ok(()));
    assert_eq!(root.remove("./foo//.//f3").await, Ok(()));
    assert_eq!(root.remove("./foo").await, Ok(()));
    assert!(ramfs.root_dir_node().get_entries().is_empty());
}

#[test]
fn test_ramfs() {
    use core::future::Future;
    let waker = core::task::Waker::noop();
    let mut cx = Context::from_waker(&waker);

    let _ = Box::pin(test()).as_mut().poll(&mut cx);
}
