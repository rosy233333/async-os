#![no_std]

use lazy_init::LazyInit;
use xmas_elf::symbol_table::{DynEntry64, Entry};

mod vdso_func;
pub use vdso_func::*;

static VDSO_DATA: LazyInit<&'static [u8]> = LazyInit::new();
static VDSO_DYN_SYM_TABLE: LazyInit<&'static [DynEntry64]> = LazyInit::new();

pub fn init() {
    // log::info!("init start");
    // #[cfg(feature = "kernel")]
    // {
    //     use vdso::VDSO_INFO;
    //     VDSO_DATA.init_by(VDSO_INFO.elf_data);
    // }
    #[cfg(not(feature = "kernel"))]
    {
        extern "C" {
            fn getauxval(key: u64) -> u64;
        }
        const AT_SYSINFO_EHDR: u64 = 33;

        let vdso_base = unsafe { getauxval(AT_SYSINFO_EHDR) };
        VDSO_DATA.init_by(unsafe { core::slice::from_raw_parts(vdso_base as *const u8, 0x1000) });
    }
    log::info!("init VDSO_DATA successful");
    log::info!("VDSO_DATA: {:016x}", VDSO_DATA.as_ptr() as usize);

    let vdso_elf = xmas_elf::ElfFile::new(&VDSO_DATA).unwrap();
    if let Some(dyn_sym_table) = vdso_elf.find_section_by_name(".dynsym") {
        VDSO_DYN_SYM_TABLE.init_by(match dyn_sym_table.get_data(&vdso_elf) {
            Ok(xmas_elf::sections::SectionData::DynSymbolTable64(dyn_sym_table)) => dyn_sym_table,
            _ => panic!("Invalid data in .dynsym section"),
        });
    }
    else {
        panic!("Can't find dyn_sym_table in vdso!");
    }
    log::info!("init VDSO_DYN_SYM_TABLE successful");
    log::info!("add_scheduler: {:016x}", get_fn_ptr("__vdso_add_scheduler"));
    log::info!("delete_scheduler: {:016x}", get_fn_ptr("__vdso_delete_scheduler"));
    log::info!("add_task: {:016x}", get_fn_ptr("__vdso_add_task"));
    log::info!("clear_current: {:016x}", get_fn_ptr("__vdso_clear_current"));
    log::info!("pick_next_task: {:016x}", get_fn_ptr("__vdso_pick_next_task"));
}

pub(crate) fn get_fn_ptr(name: &str) -> usize {
    let vdso_elf = xmas_elf::ElfFile::new(&VDSO_DATA).unwrap();
    let offset = VDSO_DYN_SYM_TABLE.iter().find(|&dynsym| {
        dynsym.get_name(&vdso_elf).unwrap() == name
    }).unwrap().value();

    let vdso_base = VDSO_DATA.as_ptr() as usize;
    vdso_base + (offset as usize)
}