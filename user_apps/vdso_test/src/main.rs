use std::process::Command;

use libc::{wait, WIFEXITED};
use xmas_elf::symbol_table::Entry;

const AT_SYSINFO_EHDR: u64 = 33;

extern "C" {
    fn getauxval(key: u64) -> u64;
}

const PAGE_SIZE_4K: usize = 4096;
const VDSO_SIZE: usize =
    ((include_bytes!("../../../vdso/libvdsoexample.so").len() - 1) / PAGE_SIZE_4K + 1)
        * PAGE_SIZE_4K;

fn main() {
    // let env = env_logger::Env::default().filter_or("LOG", "debug");
    // env_logger::init_from_env(env);
    let vdso_base = unsafe { getauxval(AT_SYSINFO_EHDR) };
    println!("{:#X?}", vdso_base);

    let vdso_data = unsafe { core::slice::from_raw_parts(vdso_base as *const u8, VDSO_SIZE) };
    let vdso_elf = xmas_elf::ElfFile::new(vdso_data).unwrap();

    unsafe {
        api::init_vdso_vtable(vdso_base, &vdso_elf);
    }

    // // 单进程测试
    // unsafe {
    //     test_vdso();
    // }

    // 多进程测试，目前仍有bug
    match unsafe { libc::fork() } {
        0 => {
            // 子进程
            unsafe { test_vdso_child() }
        }
        _ => {
            // 父进程

            // 等待子进程结束
            let mut status = 0;
            unsafe {
                wait(&mut status);
                WIFEXITED(status)
                    .then(|| println!("Child process exited successfully."))
                    .unwrap_or_else(|| panic!("Child process did not exit successfully."));
            }

            unsafe { test_vdso_parent() }
        }
    }
}

/// SAFETY: 调用该函数前需要先调用api::init_vdso_vtable。
unsafe fn test_vdso_child() {
    println!("Testing vDSO in child process...");
    assert_eq!(api::get_shared().i, 1); // 共享数据已被内核修改
    api::set_shared(2);
    assert_eq!(api::get_shared().i, 2);
    assert_eq!(api::get_private().i, 0); // 私有数据不应被内核的修改影响
    api::set_private(2);
    assert_eq!(api::get_private().i, 2);
    println!("Test passed!");
}

/// SAFETY: 调用该函数前需要先调用api::init_vdso_vtable。
unsafe fn test_vdso_parent() {
    println!("Testing vDSO in parent process...");
    assert_eq!(api::get_shared().i, 2); // 共享数据已被子进程修改
    api::set_shared(3);
    assert_eq!(api::get_shared().i, 3);
    assert_eq!(api::get_private().i, 0); // 私有数据不应被内核或子进程的修改影响
    api::set_private(3);
    assert_eq!(api::get_private().i, 3);
    println!("Test passed!");
}

/// SAFETY: 调用该函数前需要先调用api::init_vdso_vtable。
unsafe fn test_vdso() {
    println!("Testing vDSO in userspace...");
    assert_eq!(api::get_shared().i, 1); // 共享数据已被内核修改
    api::set_shared(2);
    assert_eq!(api::get_shared().i, 2);
    assert_eq!(api::get_private().i, 0); // 私有数据不应被内核的修改影响
    api::set_private(2);
    assert_eq!(api::get_private().i, 2);
    println!("Test passed!");
}
