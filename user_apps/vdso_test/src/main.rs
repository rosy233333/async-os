use xmas_elf::symbol_table::Entry;

fn main() {
    let vdso_data = unsafe { core::slice::from_raw_parts(0x40aa000 as *const u8, 0x1000) };
    let vdso_elf = xmas_elf::ElfFile::new(vdso_data).unwrap();
    if let Some(dyn_sym_table) = vdso_elf.find_section_by_name(".dynsym") {
        let dyn_sym_table = match dyn_sym_table.get_data(&vdso_elf) {
            Ok(xmas_elf::sections::SectionData::DynSymbolTable64(dyn_sym_table)) => dyn_sym_table,
            _ => panic!("Invalid data in .dynsym section"),
        };
        for dynsym in dyn_sym_table {
            let name = dynsym.get_name(&vdso_elf).unwrap();
            if name.starts_with("__vdso") {
                println!("{}: {:?}", name, dynsym.value(),);
                let fn_ptr = 0x40aa000 + dynsym.value();
                let f: fn() -> usize = unsafe { core::mem::transmute(fn_ptr) };
                let val = f();
                println!("val: {:#X?}", val);
            }
        }
    }
}
