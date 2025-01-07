#![no_std]

extern crate alloc;

use alloc::sync::Arc;
use spinlock::{SpinNoIrq, SpinRaw};
pub type KScheduler = taskctx::Scheduler;
pub type KTaskRef = taskctx::TaskRef;
pub type UScheduler = utaskctx::Scheduler;
pub type UTaskRef = utaskctx::TaskRef;

pub fn init_kschedulers() {
    todo!("初始化每个CPU的内核调度器")
}

/// 用户调度器使用Arc<SpinRaw<UScheduler>>类型，内核调度器使用Arc<SpinNoIrq<KScheduler>>类型
pub fn set_uscheduler(uscheduler: Arc<SpinRaw<UScheduler>>, ktask: KTaskRef) {
    assert!(check_ktask_scheduler(ktask));
    todo!("设置当前核心的当前用户调度器，并将其与内核任务关联。")
}

pub fn delete_uscheduler(uscheduler: Arc<SpinRaw<UScheduler>>) {
    todo!("删除用户调度器")
}

pub enum CurrentScheduler {
    Kernel(Arc<SpinNoIrq<KScheduler>>),
    User(Arc<SpinRaw<UScheduler>>)
}

pub fn get_current_scheduler() -> CurrentScheduler {
    todo!("获取当前核心正在使用的调度器")
}

pub fn into_kernel() {
    todo!("当前核心使用内核调度器")
}

/// SAFETY: 需要在当前CPU上先调用set_uscheduler设置用户调度器，且该调度器还未被delete_uscheduler删除。
pub unsafe fn into_user() {
    todo!("当前核心使用用户调度器")
}

pub fn add_ktask(task: KTaskRef) {
    assert!(check_ktask_scheduler(task));
    todo!("调用Scheduler::add_task将任务加入当前核心调度器");
}

pub fn add_utask(task: UTaskRef) {
    assert!(check_utask_scheduler(task));
    todo!("调用Scheduler::add_task将任务加入当前核心调度器");
}

pub fn put_prev_ktask(task: KTaskRef, preempt: bool) {
    assert!(check_ktask_scheduler(task));
    todo!("调用Scheduler::put_prev_task将任务加入当前核心调度器");
}

pub fn put_prev_utask(task: UTaskRef, preempt: bool) {
    assert!(check_utask_scheduler(task));
    todo!("调用Scheduler::put_prev_task将任务加入当前核心调度器");
}

pub fn pick_next_utask() -> Option<UTaskRef> {
    todo!("从当前核心调度器取出任务")
}

pub fn pick_next_ktask() -> Option<KTaskRef> {
    todo!("从当前核心调度器取出任务")
}

fn check_ktask_scheduler(task: KTaskRef) -> bool {
    todo!("检查给定任务是否属于当前调度器")
}

fn check_utask_scheduler(task: UTaskRef) -> bool {
    todo!("检查给定任务是否属于当前调度器")
}