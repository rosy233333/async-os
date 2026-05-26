// use std::process::Command;

// use libc::{wait, WIFEXITED};
// use xmas_elf::symbol_table::Entry;

// const AT_SYSINFO_EHDR: u64 = 33;

// extern "C" {
//     fn getauxval(key: u64) -> u64;
// }

// const PAGE_SIZE_4K: usize = 4096;
// const VDSO_SIZE: usize =
//     ((include_bytes!("../../../vdso_output/libvdsoexample.so").len() - 1) / PAGE_SIZE_4K + 1)
//         * PAGE_SIZE_4K;

// fn main() {
//     // let env = env_logger::Env::default().filter_or("LOG", "debug");
//     // env_logger::init_from_env(env);
//     let vdso_base = unsafe { getauxval(AT_SYSINFO_EHDR) };
//     println!("{:#X?}", vdso_base);

//     unsafe {
//         libvdsoexample::init_vdso_vtable(vdso_base);
//     }

//     // // 单进程测试
//     // unsafe {
//     //     test_vdso();
//     // }

//     // 多进程测试，目前仍有bug
//     match unsafe { libc::fork() } {
//         0 => {
//             // 子进程
//             unsafe { test_vdso_child() }
//         }
//         _ => {
//             // 父进程

//             // 等待子进程结束
//             let mut status = 0;
//             unsafe {
//                 wait(&mut status);
//                 WIFEXITED(status)
//                     .then(|| println!("Child process exited successfully."))
//                     .unwrap_or_else(|| panic!("Child process did not exit successfully."));
//             }

//             unsafe { test_vdso_parent() }
//         }
//     }
// }

// /// SAFETY: 调用该函数前需要先调用libvdsoexample::init_vdso_vtable。
// unsafe fn test_vdso_child() {
//     println!("Testing vDSO in child process...");
//     assert_eq!(libvdsoexample::get_shared().i, 1); // 共享数据已被内核修改
//     libvdsoexample::set_shared(2);
//     assert_eq!(libvdsoexample::get_shared().i, 2);
//     assert_eq!(libvdsoexample::get_private().i, 0); // 私有数据不应被内核的修改影响
//     libvdsoexample::set_private(2);
//     assert_eq!(libvdsoexample::get_private().i, 2);
//     println!("Test passed!");
// }

// /// SAFETY: 调用该函数前需要先调用libvdsoexample::init_vdso_vtable。
// unsafe fn test_vdso_parent() {
//     println!("Testing vDSO in parent process...");
//     assert_eq!(libvdsoexample::get_shared().i, 2); // 共享数据已被子进程修改
//     libvdsoexample::set_shared(3);
//     assert_eq!(libvdsoexample::get_shared().i, 3);
//     assert_eq!(libvdsoexample::get_private().i, 0); // 私有数据不应被内核或子进程的修改影响
//     libvdsoexample::set_private(3);
//     assert_eq!(libvdsoexample::get_private().i, 3);
//     println!("Test passed!");
// }

// /// SAFETY: 调用该函数前需要先调用libvdsoexample::init_vdso_vtable。
// unsafe fn test_vdso() {
//     println!("Testing vDSO in userspace...");
//     assert_eq!(libvdsoexample::get_shared().i, 1); // 共享数据已被内核修改
//     libvdsoexample::set_shared(2);
//     assert_eq!(libvdsoexample::get_shared().i, 2);
//     assert_eq!(libvdsoexample::get_private().i, 0); // 私有数据不应被内核的修改影响 // 当前这个通不过，左边变成2了
//     libvdsoexample::set_private(2);
//     assert_eq!(libvdsoexample::get_private().i, 2);
//     println!("Test passed!");
// }

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

fn main() {}