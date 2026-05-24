#![no_std]
#![no_main]

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

extern crate async_std;
extern crate trampoline;

#[async_std::async_main]
async fn main() -> isize {
    async_std::println!("user_boot");
    // 初始化文件系统
    trampoline::fs_init().await;
    // for testcase in BUSYBOX_TESTCASES {
    for testcase in TESTCASES {
        let task = trampoline::init_user(get_args(testcase.as_bytes()), &get_envs().await)
            .await
            .unwrap();
        trampoline::wait(&task).await;
        async_std::println!("task count {}", alloc::sync::Arc::strong_count(&task));
    }
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

#[allow(unused)]
/// 分割命令行参数
fn get_args(command_line: &[u8]) -> Vec<String> {
    let mut args = Vec::new();
    // 需要判断是否存在引号，如busybox_cmd.txt的第一条echo指令便有引号
    // 若有引号时，不能把引号加进去，同时要注意引号内的空格不算是分割的标志
    let mut in_quote = false;
    let mut arg_start = 0; // 一个新的参数的开始位置
    for pos in 0..command_line.len() {
        if command_line[pos] == b'\"' {
            in_quote = !in_quote;
        }
        if command_line[pos] == b' ' && !in_quote {
            // 代表要进行分割
            // 首先要防止是否有空串
            if arg_start != pos {
                args.push(
                    core::str::from_utf8(&command_line[arg_start..pos])
                        .unwrap()
                        .to_string(),
                );
            }
            arg_start = pos + 1;
        }
    }
    // 最后一个参数
    if arg_start != command_line.len() {
        args.push(
            core::str::from_utf8(&command_line[arg_start..])
                .unwrap()
                .to_string(),
        );
    }
    args
}

#[allow(dead_code)]
const BUSYBOX_TESTCASES: &[&str] = &[
    "busybox sh busybox_testcode.sh",
    "busybox sh lua_testcode.sh",
    "libctest_testcode.sh",
];

#[allow(dead_code)]
const TESTCASES: &[&str] = &[
    // "batch_syscall",
    // "syscall_test",
    "vdso_test",
    // "hello_world",
    // "pipetest",
    // "std_thread_test",
];

// struct MemIfImpl;

// #[crate_interface::impl_interface]
// impl libvdsoexample::MemIf for MemIfImpl {
//     #[doc = " 在地址空间中分配用于vDSO和vVAR的虚存区域（不需同时分配物理页面），返回指向首地址的指针。"]
//     #[doc = " "]
//     #[doc = " 保证size为build_vdso传入的config.page_size的整数倍。"]
//     #[doc = " 要求返回的地址也为config.page_size的整数倍。"]
//     fn valloc(vspace: usize, size: usize) -> *mut u8 {
//         todo!()
//     }

//     #[doc = " 分配多块用于vDSO和vVAR的连续物理页，返回`PhysPagePtr`。"]
//     #[doc = " "]
//     #[doc = " 保证size为build_vdso传入的config.page_size的整数倍。"]
//     #[doc = ""]
//     #[doc = " 若需要实现vDSO和vVAR在多地址空间的共享，则需要在分配时使这块空间可被共享（即，可被多次`map`）。"]
//     fn ppage_alloc(size: usize) -> libvdsoexample::PhysPagePtr {
//         todo!()
//     }

//     #[doc = " 从`alloc`返回的虚存区域中，映射其中一块到某个物理页面并设置权限。"]
//     #[doc = " "]
//     #[doc = " 被映射的物理页面可能和其它地址空间共享，也可能由这个地址空间独占。"]
//     #[doc = " "]
//     #[doc = " 保证vaddr对齐到build_vdso传入的config.page_size；len为config.page_size的整数倍。"]
//     #[doc = ""]
//     #[doc = " `flags`可能包含：READ、WRITE、EXECUTE、USER。"]
//     fn map(
//         vspace: usize,
//         vaddr: *mut u8,
//         ppage: libvdsoexample::PhysPagePtr,
//         size: usize,
//         flags: libvdsoexample::MappingFlags,
//     ) {
//         todo!()
//     }

//     #[doc = " 重新设置已映射好的，虚拟首地址为`vspace`区域的权限。"]
//     #[doc = " "]
//     #[doc = " 保证vaddr对齐到build_vdso传入的config.page_size。"]
//     fn change_protect(
//         vspace: usize,
//         vaddr: *mut u8,
//         size: usize,
//         flags: libvdsoexample::MappingFlags,
//     ) {
//         todo!()
//     }

//     #[doc = " 获取`vspace`空间中`vaddr`地址对应的内核虚拟地址。"]
//     #[doc = " （也就是当前代码可以直接访问的地址）"]
//     fn get_kernel_vaddr(vspace: usize, vaddr: *mut u8) -> *mut u8 {
//         todo!()
//     }

//     #[doc = " 复制物理页指针，复制前后指向同一块物理页。复制后，参数和返回值对应的两个指针均需可用。"]
//     #[doc = " "]
//     #[doc = " 如果物理页使用RAII管理，则需调用其`clone`方法。"]
//     #[doc = " "]
//     #[doc = " 如果物理页不使用RAII管理，则可以直接返回参数。"]
//     fn ppage_clone(ppage: libvdsoexample::PhysPagePtr) -> libvdsoexample::PhysPagePtr {
//         todo!()
//     }
// }
