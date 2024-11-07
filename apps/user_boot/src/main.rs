#![no_std]
#![no_main]

use alloc::{string::String, vec, vec::Vec};

extern crate async_std;
extern crate trampoline;

#[async_std::async_main]
async fn main() -> i32 {
    async_std::println!("user_boot");
    // 初始化文件系统
    trampoline::fs_init().await;
    let task = trampoline::init_user(vec!["busybox".into(), "sh".into(), "busybox_testcode.sh".into()], &get_envs().await)
        .await
        .unwrap();
    // let task = trampoline::init_user(vec!["hello".into()], &get_envs()).await.unwrap();
    trampoline::wait(&task).await;
    async_std::println!("task count {}", alloc::sync::Arc::strong_count(&task));
    0
}

/// Now the environment variables are hard coded, we need to read the file "/etc/environment" to get the environment variables
pub async fn get_envs() -> Vec<String> {
    // Const string for environment variables
    let mut envs:Vec<String> = vec![
        "SHLVL=1".into(),
        "PWD=/".into(),
        "GCC_EXEC_PREFIX=/riscv64-linux-musl-native/bin/../lib/gcc/".into(),
        "COLLECT_GCC=./riscv64-linux-musl-native/bin/riscv64-linux-musl-gcc".into(),
        "COLLECT_LTO_WRAPPER=/riscv64-linux-musl-native/bin/../libexec/gcc/riscv64-linux-musl/11.2.1/lto-wrapper".into(),
        "COLLECT_GCC_OPTIONS='-march=rv64gc' '-mabi=lp64d' '-march=rv64imafdc' '-dumpdir' 'a.'".into(),
        "LIBRARY_PATH=/lib/".into(),
        "LD_LIBRARY_PATH=/lib/".into(),
        "LD_DEBUG=files".into(),
    ];
    // read the file "/etc/environment"
    // if exist, then append the content to envs
    // else set the environment variable to default value
    if let Some(environment_vars) = async_std::fs::read_to_string("/etc/environment").await.ok() {
        envs.push(environment_vars);
    } else {
        envs.push("PATH=/usr/sbin:/usr/bin:/sbin:/bin".into());
    }
    envs
}
