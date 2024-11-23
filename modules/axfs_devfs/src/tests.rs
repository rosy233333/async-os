use std::sync::Arc;

use async_vfs::{AsyncVfsNodeOps, VfsError, VfsNodeType, VfsResult};

use crate::*;

async fn test_devfs_ops(devfs: &DeviceFileSystem) -> VfsResult {
    const N: usize = 32;
    let mut buf = [1; N];

    let root = devfs.root_dir();
    assert!(root.get_attr().await?.is_dir());
    assert_eq!(root.get_attr().await?.file_type(), VfsNodeType::Dir);
    assert_eq!(
        root.clone().lookup("urandom").err(),
        Some(VfsError::NotFound)
    );
    assert_eq!(
        root.clone().lookup("zero/").err(),
        Some(VfsError::NotADirectory)
    );

    let node = root.lookup("////null")?;
    assert_eq!(node.get_attr().await?.file_type(), VfsNodeType::CharDevice);
    assert!(!node.get_attr().await?.is_dir());
    assert_eq!(node.get_attr().await?.size(), 0);
    assert_eq!(node.read_at(0, &mut buf).await?, 0);
    assert_eq!(buf, [1; N]);
    assert_eq!(node.write_at(N as _, &buf).await?, N);
    assert_eq!(node.lookup("/").err(), Some(VfsError::NotADirectory));

    let node = devfs.root_dir().lookup(".///.//././/.////zero")?;
    assert_eq!(node.get_attr().await?.file_type(), VfsNodeType::CharDevice);
    assert!(!node.get_attr().await?.is_dir());
    assert_eq!(node.get_attr().await?.size(), 0);
    assert_eq!(node.read_at(10, &mut buf).await?, N);
    assert_eq!(buf, [0; N]);
    assert_eq!(node.write_at(0, &buf).await?, N);

    let foo = devfs.root_dir().lookup(".///.//././/.////foo")?;
    assert!(foo.get_attr().await?.is_dir());
    assert_eq!(
        foo.read_at(10, &mut buf).await.err(),
        Some(VfsError::IsADirectory)
    );
    assert!(Arc::ptr_eq(
        &foo.clone().lookup("/f2")?,
        &devfs.root_dir().lookup(".//./foo///f2")?,
    ));
    assert_eq!(
        foo.clone()
            .lookup("/bar//f1")?
            .get_attr()
            .await?
            .file_type(),
        VfsNodeType::CharDevice
    );
    assert_eq!(
        foo.lookup("/bar///")?.get_attr().await?.file_type(),
        VfsNodeType::Dir
    );

    Ok(())
}

async fn test_get_parent(devfs: &DeviceFileSystem) -> VfsResult {
    let root = devfs.root_dir();
    assert!(root.parent().is_none());

    let node = root.clone().lookup("null")?;
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
        &root.clone().lookup("///foo//bar///../f2")?,
        &root.lookup("foo/.//f2")?,
    ));

    Ok(())
}

#[test]
fn test_devfs() {
    // .
    // ├── foo
    // │   ├── bar
    // │   │   └── f1 (null)
    // │   └── f2 (zero)
    // ├── null
    // └── zero
    use alloc::boxed::Box;
    use core::{
        future::Future,
        task::{Context, Waker},
    };

    let devfs = DeviceFileSystem::new();
    devfs.add("null", Arc::new(NullDev));
    devfs.add("zero", Arc::new(ZeroDev));

    let dir_foo = devfs.mkdir("foo");
    dir_foo.add("f2", Arc::new(ZeroDev));
    let dir_bar = dir_foo.mkdir("bar");
    dir_bar.add("f1", Arc::new(NullDev));
    let waker = Waker::noop();
    let mut cx = Context::from_waker(&waker);

    let _ = Box::pin(test_devfs_ops(&devfs)).as_mut().poll(&mut cx);
    let _ = Box::pin(test_get_parent(&devfs)).as_mut().poll(&mut cx);
}
