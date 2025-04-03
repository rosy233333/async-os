extern crate xmas_elf;

use axconfig::PHYS_VIRT_OFFSET;
use core::mem::size_of;
use core::ptr::copy_nonoverlapping;
use heapless::Vec;
use vdso::get_vdso_base_end;
use xmas_elf::program::SegmentData;
use xmas_elf::symbol_table::Entry;

const PAGE_SIZE: usize = 0x1000;
const VDSO_VIRT_BASE: usize = 0xffff_ffff_c000_0000;

#[link_section = ".data.boot_page_table"]
static mut SECOND_PT_SV39: [u64; 512] = [0; 512];

#[link_section = ".data.boot_page_table"]
static mut THIRD_PT_SV39: [u64; 512] = [0; 512];

/// setup the page table for vDSO
/// the virtual address of vDSO is 0xffff_ffff_c000_0000 + vdso_data_size
pub(crate) unsafe fn init_vdso_page_table(boot_page_table: *mut [u64; 512]) {
    let (sdata, edata, base, end) = get_vdso_base_end();
    let sdata = sdata;
    let edata = edata;
    let base = base;
    let end = end;
    let mut pte_idx = 0;
    for data_base in (sdata..edata).step_by(PAGE_SIZE) {
        THIRD_PT_SV39[pte_idx] = (data_base >> 2) | 0xef;
        pte_idx += 1;
    }
    for text_base in (base..end).step_by(PAGE_SIZE) {
        THIRD_PT_SV39[pte_idx] = (text_base >> 2) | 0xef;
        pte_idx += 1;
    }
    // setup third page table
    let page_table_4k = (&raw const THIRD_PT_SV39 as u64);
    SECOND_PT_SV39[0x0] = (page_table_4k >> 2) | 0x1;
    // setup secondary page table
    let page_table_2m = (&raw const SECOND_PT_SV39 as u64);
    (*boot_page_table)[0x1ff] = (page_table_2m >> 2) | 0x1;
}

pub(crate) fn relocate_vdso() {
    let (sdata, edata, base, end) = get_vdso_base_end();
    let vdso_text_virt_base = VDSO_VIRT_BASE + (edata - sdata) as usize;
    log::warn!("vdso text base: 0x{:x}", vdso_text_virt_base);
    let vdso_data_size = (edata - sdata) as usize;
    let vdso_text_size = (end - base) as usize;
    let elf_data =
        unsafe { core::slice::from_raw_parts(base as *const u8, end as usize - base as usize) };

    let elf = xmas_elf::ElfFile::new(&elf_data).expect("Error parsing app ELF file.");
    if let Some(interp) = elf
        .program_iter()
        .find(|ph| ph.get_type() == Ok(xmas_elf::program::Type::Interp))
    {
        let _interp = match interp.get_data(&elf) {
            Ok(SegmentData::Undefined(data)) => data,
            _ => panic!("Invalid data in Interp Elf Program Header"),
        };

        // let interp_path = from_utf8(interp).expect("Interpreter path isn't valid UTF-8");
        // // remove trailing '\0'
        // let _interp_path = interp_path.trim_matches(char::from(0)).to_string();
    }
    let elf_base_addr = Some(vdso_text_virt_base);
    let relocate_pairs = get_relocate_pairs(&elf, elf_base_addr);

    for relocate_pair in relocate_pairs {
        let src: usize = relocate_pair.src.into();
        let dst: usize = relocate_pair.dst.into();
        let count = relocate_pair.count;
        log::warn!(
            "Relocate: src: 0x{:x}, dst: 0x{:x}, count: {}",
            src,
            dst,
            count
        );
        unsafe { copy_nonoverlapping(src.to_ne_bytes().as_ptr(), dst as *mut u8, count) }
    }
}

#[derive(Debug, Default, Clone, Copy)]
/// To describe the relocation pair in the ELF
pub struct RelocatePair {
    /// the source address of the relocation
    pub src: usize,
    /// the destination address of the relocation
    pub dst: usize,
    /// the set of bits affected by this relocation
    pub count: usize,
}

impl RelocatePair {
    pub const NULL: RelocatePair = RelocatePair {
        src: 0,
        dst: 0,
        count: 0,
    };
}

const R_RISCV_32: u32 = 1;
const R_RISCV_64: u32 = 2;
const R_RISCV_RELATIVE: u32 = 3;
const R_JUMP_SLOT: u32 = 5;
const TLS_DTPREL32: u32 = 8;
const TLS_DTV_OFFSET: usize = 0x800;
/// To parse the elf file and get the relocate pairs
///
/// # Arguments
///
/// * `elf` - The elf file
/// * `elf_base_addr` - The base address of the elf file if the file will be loaded to the memory
pub fn get_relocate_pairs(
    elf: &xmas_elf::ElfFile,
    elf_base_addr: Option<usize>,
) -> Vec<RelocatePair, 10> {
    let elf_header = elf.header;
    let magic = elf_header.pt1.magic;
    assert_eq!(magic, [0x7f, 0x45, 0x4c, 0x46], "invalid elf!");
    let mut pairs = Vec::<RelocatePair, 10>::new();
    // Some elf will load ELF Header (offset == 0) to vaddr 0. In that case, base_addr will be added to all the LOAD.
    let base_addr: usize = if let Some(header) = elf
        .program_iter()
        .find(|ph| ph.get_type() == Ok(xmas_elf::program::Type::Load))
    {
        // Loading ELF Header into memory.
        let vaddr = header.virtual_addr() as usize;

        if vaddr == 0 {
            if let Some(addr) = elf_base_addr {
                addr
            } else {
                panic!("ELF Header is loaded to vaddr 0, but no base_addr is provided");
            }
        } else {
            0
        }
    } else {
        0
    };
    if let Some(rela_dyn) = elf.find_section_by_name(".rela.dyn") {
        let data = match rela_dyn.get_data(elf) {
            Ok(xmas_elf::sections::SectionData::Rela64(data)) => data,
            _ => panic!("Invalid data in .rela.dyn section"),
        };

        if let Some(dyn_sym_table) = elf.find_section_by_name(".dynsym") {
            let dyn_sym_table = match dyn_sym_table.get_data(elf) {
                Ok(xmas_elf::sections::SectionData::DynSymbolTable64(dyn_sym_table)) => {
                    dyn_sym_table
                }
                _ => panic!("Invalid data in .dynsym section"),
            };

            for entry in data {
                let dyn_sym = &dyn_sym_table[entry.get_symbol_table_index() as usize];
                let destination = base_addr + entry.get_offset() as usize;
                let symbol_value = dyn_sym.value() as usize; // Represents the value of the symbol whose index resides in the relocation entry.
                let addend = entry.get_addend() as usize; // Represents the addend used to compute the value of the relocatable field.
                let _symbol_name = dyn_sym.get_name(elf).unwrap();
                match entry.get_type() {
                    R_RISCV_32 => {
                        if dyn_sym.shndx() == 0 {
                            let name = dyn_sym.get_name(elf).unwrap();
                            panic!(r#"Symbol "{}" not found"#, name);
                        }
                        pairs
                            .push(RelocatePair {
                                src: symbol_value + addend,
                                dst: destination,
                                count: 4,
                            })
                            .unwrap()
                    }
                    R_RISCV_64 => {
                        if dyn_sym.shndx() == 0 {
                            let name = dyn_sym.get_name(elf).unwrap();
                            panic!(r#"Symbol "{}" not found"#, name);
                        }
                        pairs
                            .push(RelocatePair {
                                src: symbol_value + addend,
                                dst: destination,
                                count: 8,
                            })
                            .unwrap()
                    }
                    R_RISCV_RELATIVE => pairs
                        .push(RelocatePair {
                            src: base_addr.wrapping_add(addend),
                            dst: destination,
                            count: size_of::<usize>() / size_of::<u8>(),
                        })
                        .unwrap(),
                    R_JUMP_SLOT => {
                        if dyn_sym.shndx() == 0 {
                            let name = dyn_sym.get_name(elf).unwrap();
                            panic!(r#"Symbol "{}" not found"#, name);
                        }
                        pairs
                            .push(RelocatePair {
                                src: symbol_value,
                                dst: destination,
                                count: size_of::<usize>() / size_of::<u8>(),
                            })
                            .unwrap()
                    }
                    TLS_DTPREL32 => pairs
                        .push(RelocatePair {
                            src: symbol_value + addend - TLS_DTV_OFFSET,
                            dst: destination,
                            count: 4,
                        })
                        .unwrap(),
                    other => panic!("Unknown relocation type: {}", other),
                }
            }
        }
    }

    // Relocate .rela.plt sections
    if let Some(rela_plt) = elf.find_section_by_name(".rela.plt") {
        let data = match rela_plt.get_data(elf) {
            Ok(xmas_elf::sections::SectionData::Rela64(data)) => data,
            _ => panic!("Invalid data in .rela.plt section"),
        };
        if elf.find_section_by_name(".dynsym").is_some() {
            let dyn_sym_table = match elf
                .find_section_by_name(".dynsym")
                .expect("Dynamic Symbol Table not found for .rela.plt section")
                .get_data(elf)
            {
                Ok(xmas_elf::sections::SectionData::DynSymbolTable64(dyn_sym_table)) => {
                    dyn_sym_table
                }
                _ => panic!("Invalid data in .dynsym section"),
            };

            for entry in data {
                let dyn_sym = &dyn_sym_table[entry.get_symbol_table_index() as usize];
                let destination = base_addr + entry.get_offset() as usize;
                match entry.get_type() {
                    R_JUMP_SLOT => {
                        let symbol_value = if dyn_sym.shndx() != 0 {
                            dyn_sym.value() as usize
                        } else {
                            let name = dyn_sym.get_name(elf).unwrap();
                            panic!(r#"Symbol "{}" not found"#, name);
                        }; // Represents the value of the symbol whose index resides in the relocation entry.
                        pairs
                            .push(RelocatePair {
                                src: symbol_value + base_addr,
                                dst: destination,
                                count: size_of::<usize>(),
                            })
                            .unwrap();
                    }
                    other => panic!("Unknown relocation type: {}", other),
                }
            }
        }
    }

    pairs
}
