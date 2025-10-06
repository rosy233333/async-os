use std::process::Command;

use libc::{wait, WIFEXITED};
use xmas_elf::symbol_table::Entry;

const AT_SYSINFO_EHDR: u64 = 33;

extern "C" {
    fn getauxval(key: u64) -> u64;
}

const PAGE_SIZE_4K: usize = 4096;
const VDSO_SIZE: usize =
    ((include_bytes!("../../../vdso_output/libvdsoexample.so").len() - 1) / PAGE_SIZE_4K + 1)
        * PAGE_SIZE_4K;

fn main() {
    // let env = env_logger::Env::default().filter_or("LOG", "debug");
    // env_logger::init_from_env(env);
    let vdso_base = unsafe { getauxval(AT_SYSINFO_EHDR) };
    println!("{:#X?}", vdso_base);

    unsafe {
        libvdsoexample::init_vdso_vtable(vdso_base);
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

/// SAFETY: 调用该函数前需要先调用libvdsoexample::init_vdso_vtable。
unsafe fn test_vdso_child() {
    println!("Testing vDSO in child process...");
    assert_eq!(libvdsoexample::get_shared().i, 1); // 共享数据已被内核修改
    libvdsoexample::set_shared(2);
    assert_eq!(libvdsoexample::get_shared().i, 2);
    assert_eq!(libvdsoexample::get_private().i, 0); // 私有数据不应被内核的修改影响
    libvdsoexample::set_private(2);
    assert_eq!(libvdsoexample::get_private().i, 2);
    println!("Test passed!");
}

/// SAFETY: 调用该函数前需要先调用libvdsoexample::init_vdso_vtable。
unsafe fn test_vdso_parent() {
    println!("Testing vDSO in parent process...");
    assert_eq!(libvdsoexample::get_shared().i, 2); // 共享数据已被子进程修改
    libvdsoexample::set_shared(3);
    assert_eq!(libvdsoexample::get_shared().i, 3);
    assert_eq!(libvdsoexample::get_private().i, 0); // 私有数据不应被内核或子进程的修改影响
    libvdsoexample::set_private(3);
    assert_eq!(libvdsoexample::get_private().i, 3);
    println!("Test passed!");
}

/// SAFETY: 调用该函数前需要先调用libvdsoexample::init_vdso_vtable。
unsafe fn test_vdso() {
    println!("Testing vDSO in userspace...");
    assert_eq!(libvdsoexample::get_shared().i, 1); // 共享数据已被内核修改
    libvdsoexample::set_shared(2);
    assert_eq!(libvdsoexample::get_shared().i, 2);
    assert_eq!(libvdsoexample::get_private().i, 0); // 私有数据不应被内核的修改影响
    libvdsoexample::set_private(2);
    assert_eq!(libvdsoexample::get_private().i, 2);
    println!("Test passed!");
}
