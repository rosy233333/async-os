use core::{ptr::copy_nonoverlapping, str::from_utf8};

use crate::link::real_path;
use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use async_mem::MemorySet;
use axconfig::{MAX_USER_HEAP_SIZE, MAX_USER_STACK_SIZE, USER_HEAP_BASE, USER_STACK_TOP};
use axerrno::{AxError, AxResult};
use axhal::{mem::VirtAddr, paging::MappingFlags};
use elf_parser::{
    get_app_stack_region, get_auxv_vector, get_elf_entry, get_elf_segments, get_relocate_pairs,
};
use xmas_elf::program::SegmentData;

/// 返回应用程序入口，用户栈底，用户堆底
pub async fn load_app(
    mut name: String,
    mut args: Vec<String>,
    envs: &Vec<String>,
    memory_set: &mut MemorySet,
) -> AxResult<(VirtAddr, VirtAddr, VirtAddr)> {
    loop {
        if name.ends_with(".sh") {
            args = [vec![String::from("busybox"), String::from("sh")], args].concat();
            name = "busybox".to_string();
            continue;
        }
        let elf_data = if let Ok(ans) = async_fs::api::read(name.as_str()).await {
            ans
        } else {
            // exit(0)
            info!("App not found: {}", name);
            return Err(AxError::NotFound);
        };
        let elf = xmas_elf::ElfFile::new(&elf_data).expect("Error parsing app ELF file.");
        if let Some(interp) = elf
            .program_iter()
            .find(|ph| ph.get_type() == Ok(xmas_elf::program::Type::Interp))
        {
            let interp = match interp.get_data(&elf) {
                Ok(SegmentData::Undefined(data)) => data,
                _ => panic!("Invalid data in Interp Elf Program Header"),
            };

            let interp_path = from_utf8(interp).expect("Interpreter path isn't valid UTF-8");
            // remove trailing '\0'
            let interp_path = interp_path.trim_matches(char::from(0)).to_string();
            let real_interp_path = real_path(&interp_path).await;
            args = [vec![real_interp_path.clone()], args].concat();
            name = real_interp_path;
            continue;
        }
        info!("load app args: {:?} name: {}", args, name);
        let elf_base_addr = Some(0x400_0000);
        warn!("The elf base addr may be different in different arch!");
        // let (entry, segments, relocate_pairs) = parse_elf(&elf, elf_base_addr);
        let entry = get_elf_entry(&elf, elf_base_addr);
        let segments = get_elf_segments(&elf, elf_base_addr);
        let relocate_pairs = get_relocate_pairs(&elf, elf_base_addr);
        for segment in segments {
            memory_set
                .new_region(
                    segment.vaddr,
                    segment.size,
                    false,
                    segment.flags,
                    segment.data.as_deref(),
                    None,
                )
                .await;
        }

        for relocate_pair in relocate_pairs {
            let src: usize = relocate_pair.src.into();
            let dst: usize = relocate_pair.dst.into();
            let count = relocate_pair.count;
            unsafe { copy_nonoverlapping(src.to_ne_bytes().as_ptr(), dst as *mut u8, count) }
        }

        // Now map the stack and the heap
        let heap_start = VirtAddr::from(USER_HEAP_BASE);
        let heap_data = [0_u8].repeat(MAX_USER_HEAP_SIZE);
        memory_set
            .new_region(
                heap_start,
                MAX_USER_HEAP_SIZE,
                false,
                MappingFlags::READ | MappingFlags::WRITE | MappingFlags::USER,
                Some(&heap_data),
                None,
            )
            .await;
        info!(
            "[new region] user heap: [{:?}, {:?})",
            heap_start,
            heap_start + MAX_USER_HEAP_SIZE
        );

        let auxv = get_auxv_vector(&elf, elf_base_addr);

        let stack_top = VirtAddr::from(USER_STACK_TOP);
        let stack_size = MAX_USER_STACK_SIZE;

        let (stack_data, stack_bottom) = get_app_stack_region(args, envs, auxv, stack_top, stack_size);
        memory_set
            .new_region(
                stack_top,
                stack_size,
                false,
                MappingFlags::USER | MappingFlags::READ | MappingFlags::WRITE,
                Some(&stack_data),
                None,
            )
            .await;
        info!(
            "[new region] user stack: [{:?}, {:?})",
            stack_top,
            stack_top + stack_size
        );
        return Ok((entry, stack_bottom.into(), heap_start));
    }
}
