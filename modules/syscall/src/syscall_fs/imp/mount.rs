use crate::{syscall_fs::solve_path, SyscallError, SyscallResult};
use process::{
    current_executor,
    link::{raw_ptr_to_ref_str, AT_FDCWD},
};

// use super::{deal_with_path, AT_FDCWD};
use crate::syscall_fs::ctype::mount::{check_mounted, mount_fat_fs, umount_fat_fs};
extern crate alloc;
use alloc::string::ToString;
use axlog::debug;
/// 功能:挂载文件系统；
/// # Arguments
/// * `special`: *const u8, 挂载设备
/// * `dir`: *const u8, 挂载点
/// * `fs_type`: *const u8, 挂载的文件系统类型
/// * `flags`: usize, 挂载参数
/// * `data`: *const u8, 传递给文件系统的字符串参数,可为NULL
/// 返回值:成功返回0,失败返回-1
pub async fn syscall_mount(args: [usize; 6]) -> SyscallResult {
    let special = args[0] as *const u8;
    let dir = args[1] as *const u8;
    let fs_type = args[2] as *const u8;
    let _flags = args[3];
    let _data = args[4] as *const u8;
    let device_path = solve_path(AT_FDCWD, Some(special), false).await?;
    axlog::error!("syscall_mount dev: {:?}", args);
    // 这里dir必须以"/"结尾,但在shell中输入时,不需要以"/"结尾
    let mount_path = solve_path(AT_FDCWD, Some(dir), true).await?;
    axlog::error!("syscall_mount mount: {:?}", args);

    let process = current_executor().await;
    if process
        .manual_alloc_for_lazy((fs_type as usize).into())
        .await
        .is_err()
    {
        return Err(SyscallError::EINVAL);
    }

    let fs_type = unsafe { raw_ptr_to_ref_str(fs_type).to_string() };
    let mut _data_str = "".to_string();
    if !_data.is_null() {
        if process
            .manual_alloc_for_lazy((_data as usize).into())
            .await
            .is_err()
        {
            return Err(SyscallError::EINVAL);
        }
        // data可以为NULL, 必须判断, 否则会panic, 发生LoadPageFault
        _data_str = unsafe { raw_ptr_to_ref_str(_data) }.to_string();
    }
    if device_path.is_dir() {
        debug!("device_path should not be a dir");
        return Err(SyscallError::EPERM);
    }
    if !mount_path.is_dir() {
        debug!("mount_path should be a dir");
        return Err(SyscallError::EPERM);
    }

    // 如果mount_path不存在,则创建
    if !async_fs::api::path_exists(mount_path.path()).await {
        if let Err(e) = async_fs::api::create_dir(mount_path.path()).await {
            debug!("create mount path error: {:?}", e);
            return Err(SyscallError::EPERM);
        }
    }

    if fs_type != "vfat" {
        debug!("fs_type can only be vfat.");
        return Err(SyscallError::EPERM);
    }
    // 检查挂载点路径是否存在
    if !async_fs::api::path_exists(mount_path.path()).await {
        debug!("mount path not exist");
        return Err(SyscallError::EPERM);
    }
    // 查挂载点是否已经被挂载
    if check_mounted(&mount_path).await {
        debug!("mount path includes mounted fs");
        return Err(SyscallError::EPERM);
    }
    // 挂载
    if !mount_fat_fs(&device_path, &mount_path).await {
        debug!("mount error");
        return Err(SyscallError::EPERM);
    }

    Ok(0)
}

/// 功能:卸载文件系统；
/// 输入:指定卸载目录,卸载参数；
/// 返回值:成功返回0,失败返回-1
/// # Arguments
/// * `dir`: *const u8, 指定卸载目录
/// * `flags`: usize, 卸载参数
pub async fn syscall_umount(args: [usize; 6]) -> SyscallResult {
    let dir = args[0] as *const u8;
    let flags = args[1];
    let mount_path = solve_path(AT_FDCWD, Some(dir), true).await?;
    axlog::error!("syscall_umount: {:?}", args);

    if flags != 0 {
        debug!("flags unimplemented");
        return Err(SyscallError::EPERM);
    }

    // 检查挂载点路径是否存在
    if !async_fs::api::path_exists(mount_path.path()).await {
        debug!("mount path not exist");
        return Err(SyscallError::EPERM);
    }
    // 从挂载点中删除
    if !umount_fat_fs(&mount_path).await {
        debug!("umount error");
        return Err(SyscallError::EPERM);
    }

    Ok(0)
}
