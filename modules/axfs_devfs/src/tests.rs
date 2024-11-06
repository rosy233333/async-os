use std::sync::Arc;

use async_vfs::{AsyncVfsNodeOps, AsyncVfsOps, VfsError, VfsNodeType, VfsResult};

use crate::*;

async fn test_devfs_ops(devfs: &DeviceFileSystem) -> VfsResult {
    const N: usize = 32;
    let mut buf = [1; N];

    let root = devfs.root_dir().await;
    assert!(root.get_attr().await?.is_dir());
    assert_eq!(root.get_attr().await?.file_type(), VfsNodeType::Dir);
    assert_eq!(
        root.clone().lookup("urandom").await.err(),
        Some(VfsError::NotFound)
    );
    assert_eq!(
        root.clone().lookup("zero/").await.err(),
        Some(VfsError::NotADirectory)
    );

    let node = root.lookup("////null").await?;
    assert_eq!(node.get_attr().await?.file_type(), VfsNodeType::CharDevice);
    assert!(!node.get_attr().await?.is_dir());
    assert_eq!(node.get_attr().await?.size(), 0);
    assert_eq!(node.read_at(0, &mut buf).await?, 0);
    assert_eq!(buf, [1; N]);
    assert_eq!(node.write_at(N as _, &buf).await?, N);
    assert_eq!(node.lookup("/").await.err(), Some(VfsError::NotADirectory));

    let node = devfs
        .root_dir()
        .await
        .lookup(".///.//././/.////zero")
        .await?;
    assert_eq!(node.get_attr().await?.file_type(), VfsNodeType::CharDevice);
    assert!(!node.get_attr().await?.is_dir());
    assert_eq!(node.get_attr().await?.size(), 0);
    assert_eq!(node.read_at(10, &mut buf).await?, N);
    assert_eq!(buf, [0; N]);
    assert_eq!(node.write_at(0, &buf).await?, N);

    let foo = devfs
        .root_dir()
        .await
        .lookup(".///.//././/.////foo")
        .await?;
    assert!(foo.get_attr().await?.is_dir());
    assert_eq!(
        foo.read_at(10, &mut buf).await.err(),
        Some(VfsError::IsADirectory)
    );
    assert!(Arc::ptr_eq(
        &foo.clone().lookup("/f2").await?,
        &devfs.root_dir().await.lookup(".//./foo///f2").await?,
    ));
    assert_eq!(
        foo.clone()
            .lookup("/bar//f1")
            .await?
            .get_attr()
            .await?
            .file_type(),
        VfsNodeType::CharDevice
    );
    assert_eq!(
        foo.lookup("/bar///").await?.get_attr().await?.file_type(),
        VfsNodeType::Dir
    );

    Ok(())
}

async fn test_get_parent(devfs: &DeviceFileSystem) -> VfsResult {
    let root = devfs.root_dir().await;
    assert!(root.parent().await.is_none());

    let node = root.clone().lookup("null").await?;
    assert!(node.parent().await.is_none());

    let node = root.clone().lookup(".//foo/bar").await?;
    assert!(node.parent().await.is_some());
    let parent = node.parent().await.unwrap();
    assert!(Arc::ptr_eq(&parent, &root.clone().lookup("foo").await?));
    assert!(parent.lookup("bar").await.is_ok());

    // 由于 lookup 的接口进行了修改，导致外层的 Arc 指针不会相等，但内部的数据结构是指向同一份数据
    // let node = root.clone().lookup("foo/..").await?;
    // assert!(Arc::ptr_eq(&node, &root.clone().lookup(".").await?));

    // assert!(Arc::ptr_eq(
    //     &root.clone().lookup("/foo/..").await?,
    //     &devfs
    //         .root_dir()
    //         .await
    //         .lookup(".//./foo/././bar/../..")
    //         .await?,
    // ));
    // assert!(Arc::ptr_eq(
    //     &root
    //         .clone()
    //         .lookup("././/foo//./../foo//bar///..//././")
    //         .await?,
    //     &devfs.root_dir().await.lookup(".//./foo/").await?,
    // ));
    // assert!(Arc::ptr_eq(
    //     &root.clone().lookup("///foo//bar///../f2").await?,
    //     &root.lookup("foo/.//f2").await?,
    // ));

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
