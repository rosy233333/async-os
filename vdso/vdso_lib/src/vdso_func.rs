// use xmas_elf::symbol_table::Entry;
use crate::{get_fn_ptr, VDSO_DATA, VDSO_DYN_SYM_TABLE};

// 以下函数的参数、返回值规范详见vdso/cops/src/api.rs。

pub fn add_scheduler(scheduler_id: usize, ktask_info: Option<(usize, usize)>) -> bool {
    log::info!("add_scheduler: start");
    let f: fn(usize, Option<(usize, usize)>) -> bool = unsafe { core::mem::transmute(get_fn_ptr("__vdso_add_scheduler")) };
    let res = f(scheduler_id, ktask_info);
    log::info!("add_scheduler: return {:?}", res);
    res
}

pub fn delete_scheduler(scheduler_id: usize) -> bool {
    log::info!("delete_scheduler: start");
    let f: fn(usize) -> bool = unsafe { core::mem::transmute(get_fn_ptr("__vdso_delete_scheduler")) };
    let res = f(scheduler_id);
    log::info!("delete_scheduler: return {:?}", res);
    res
}

pub fn add_task(scheduler_id: usize, task_ptr: usize, default_task_prio: usize) -> bool {
    log::info!("add_task: start");
    let f: fn(usize, usize, usize) -> bool = unsafe { core::mem::transmute(get_fn_ptr("__vdso_add_task")) };
    let res = f(scheduler_id, task_ptr, default_task_prio);
    log::info!("add_task: return {:?}", res);
    res
}

pub fn clear_current(scheduler_id: usize) -> bool {
    log::info!("clear_current: start");
    let f: fn(usize) -> bool = unsafe { core::mem::transmute(get_fn_ptr("__vdso_clear_current")) };
    let res = f(scheduler_id);
    log::info!("clear_current: return {:?}", res);
    res
}

pub fn pick_next_task(scheduler_id: usize) -> Option<usize> {
    log::info!("pick_next_task: start");
    let f: fn(usize) -> Option<usize> = unsafe { core::mem::transmute(get_fn_ptr("__vdso_pick_next_task")) };
    let res = f(scheduler_id);
    log::info!("pick_next_task: return {:?}", res);
    res
}