//! 这里的与 vDSO 相关的实现可以在 build 脚本中来自动化构建，而不是手动构建出来
use crate::id::TaskId;
use xmas_elf::symbol_table::Entry;
use xmas_elf::ElfFile;

extern "C" {
    fn vdso_sdata();
    fn vdso_edata();
    fn vdso_start();
    fn vdso_end();
}

pub fn get_vdso_base_end() -> (u64, u64, u64, u64) {
    (
        vdso_sdata as _,
        vdso_edata as _,
        vdso_start as _,
        vdso_end as _,
    )
}
struct VdsoVTable {
    pub current_task: Option<fn() -> TaskId>,
    pub put_prev_task: Option<fn(task: TaskId, front: bool)>,
    pub set_current_task: Option<fn(task: TaskId)>,
    pub init: Option<fn(percpu_size: usize)>,
    pub pick_next_task: Option<fn() -> TaskId>,
    pub add_task: Option<fn(task: TaskId)>,
    pub first_add_task: Option<fn(task: TaskId)>,
}

static mut VDSO_VTABLE: VdsoVTable = VdsoVTable {
    current_task: None,
    put_prev_task: None,
    set_current_task: None,
    init: None,
    pick_next_task: None,
    add_task: None,
    first_add_task: None,
};

pub unsafe fn init_vdso_vtable(base: u64, vdso_elf: &ElfFile) {
    if let Some(dyn_sym_table) = vdso_elf.find_section_by_name(".dynsym") {
        let dyn_sym_table = match dyn_sym_table.get_data(&vdso_elf) {
            Ok(xmas_elf::sections::SectionData::DynSymbolTable64(dyn_sym_table)) => dyn_sym_table,
            _ => panic!("Invalid data in .dynsym section"),
        };
        for dynsym in dyn_sym_table {
            let name = dynsym.get_name(&vdso_elf).unwrap();
            if name == "current_task" {
                let fn_ptr = base + dynsym.value();
                log::debug!("{}: {:x}", name, fn_ptr);
                let f: fn() -> TaskId = unsafe { core::mem::transmute(fn_ptr) };
                VDSO_VTABLE.current_task = Some(f);
            }
            if name == "put_prev_task" {
                let fn_ptr = base + dynsym.value();
                log::debug!("{}: {:x}", name, fn_ptr);
                let f: fn(task: TaskId, front: bool) = unsafe { core::mem::transmute(fn_ptr) };
                VDSO_VTABLE.put_prev_task = Some(f);
            }
            if name == "set_current_task" {
                let fn_ptr = base + dynsym.value();
                log::debug!("{}: {:x}", name, fn_ptr);
                let f: fn(task: TaskId) = unsafe { core::mem::transmute(fn_ptr) };
                VDSO_VTABLE.set_current_task = Some(f);
            }
            if name == "init" {
                let fn_ptr = base + dynsym.value();
                log::debug!("{}: {:x}", name, fn_ptr);
                let f: fn(percpu_size: usize) = unsafe { core::mem::transmute(fn_ptr) };
                VDSO_VTABLE.init = Some(f);
            }
            if name == "pick_next_task" {
                let fn_ptr = base + dynsym.value();
                log::debug!("{}: {:x}", name, fn_ptr);
                let f: fn() -> TaskId = unsafe { core::mem::transmute(fn_ptr) };
                VDSO_VTABLE.pick_next_task = Some(f);
            }
            if name == "add_task" {
                let fn_ptr = base + dynsym.value();
                log::debug!("{}: {:x}", name, fn_ptr);
                let f: fn(task: TaskId) = unsafe { core::mem::transmute(fn_ptr) };
                VDSO_VTABLE.add_task = Some(f);
            }
            if name == "first_add_task" {
                let fn_ptr = base + dynsym.value();
                log::debug!("{}: {:x}", name, fn_ptr);
                let f: fn(task: TaskId) = unsafe { core::mem::transmute(fn_ptr) };
                VDSO_VTABLE.first_add_task = Some(f);
            }
        }
    }
}
    
pub fn current_task() -> TaskId {
    if let Some(f) = unsafe { VDSO_VTABLE.current_task } {
        f()
    } else {
        panic!("current_task is not initialized")
    }
}

pub fn put_prev_task(task: TaskId, front: bool) {
    if let Some(f) = unsafe { VDSO_VTABLE.put_prev_task } {
        f(task, front)
    } else {
        panic!("put_prev_task is not initialized")
    }
}

pub fn set_current_task(task: TaskId) {
    if let Some(f) = unsafe { VDSO_VTABLE.set_current_task } {
        f(task)
    } else {
        panic!("set_current_task is not initialized")
    }
}

pub fn init(percpu_size: usize) {
    if let Some(f) = unsafe { VDSO_VTABLE.init } {
        f(percpu_size)
    } else {
        panic!("init is not initialized")
    }
}

pub fn pick_next_task() -> TaskId {
    if let Some(f) = unsafe { VDSO_VTABLE.pick_next_task } {
        f()
    } else {
        panic!("pick_next_task is not initialized")
    }
}

pub fn add_task(task: TaskId) {
    if let Some(f) = unsafe { VDSO_VTABLE.add_task } {
        f(task)
    } else {
        panic!("add_task is not initialized")
    }
}

pub fn first_add_task(task: TaskId) {
    if let Some(f) = unsafe { VDSO_VTABLE.first_add_task } {
        f(task)
    } else {
        panic!("first_add_task is not initialized")
    }
}
