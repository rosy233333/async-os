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
        test_vdso();
    }
}

/// SAFETY: 调用该函数前需要先调用api::init_vdso_vtable。
unsafe fn test_vdso() {
    println!("Testing vDSO in userspace...");
    api::init();
    assert!(api::get_example().i == 1);
    api::set_example(2);
    assert!(api::get_example().i == 2);
    println!("Test passed!");
}

// fn main() {
//     let vdso_base = unsafe { getauxval(AT_SYSINFO_EHDR) };
//     println!("{:#X?}", vdso_base);

//     let vdso_data = unsafe { core::slice::from_raw_parts(vdso_base as *const u8, 0x1000) };
//     let vdso_elf = xmas_elf::ElfFile::new(vdso_data).unwrap();
//     if let Some(dyn_sym_table) = vdso_elf.find_section_by_name(".dynsym") {
//         let dyn_sym_table = match dyn_sym_table.get_data(&vdso_elf) {
//             Ok(xmas_elf::sections::SectionData::DynSymbolTable64(dyn_sym_table)) => dyn_sym_table,
//             _ => panic!("Invalid data in .dynsym section"),
//         };
//         for dynsym in dyn_sym_table {
//             let name = dynsym.get_name(&vdso_elf).unwrap();
//             if name.starts_with("__vdso") {
//                 println!("{}: {:?}", name, dynsym.value(),);
//                 let fn_ptr = 0x40aa000 + dynsym.value();
//                 let f: fn() -> usize = unsafe { core::mem::transmute(fn_ptr) };
//                 let val = f();
//                 println!("val: {:#X?}", val);
//             }
//         }
//     }
// }
