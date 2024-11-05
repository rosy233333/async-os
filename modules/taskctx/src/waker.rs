//! 这个模块手动构造 vTable，来构建 Waker
//! 构建过程中不会对 TaskRef 的引用计数增加
//! 因此，在定时器或者等待队列中注册的 Waker 不会增加引用计数
//! 从而不会产生由于 Arc 引用计数导致的性能开销
//! 为了保证 Waker 中的指针有效，需要保证 TaskRef 不会被释放
//! 这里使用的技巧是在 run_future 是：
//! 1. 若 task 返回 Ready，则会释放掉这个任务
//! 2. 若 task 返回 Pending，会调用 CurrentTask::clean_current_without_drop
//!    不释放 TaskRef，一直到 TaskRef 执行返回 Ready，将其清空，才会被释放
//!
//! 这种做法保证了 Task 模块内的代码，只有在创建时才会对引用计数增加
//! 不会因为任务阻塞而导致引用计数增加，
//! 其余对 TaskRef 引用计数的操作只会源于其余模块中的操作

use crate::{wakeup_task, Task, TaskRef};
use alloc::sync::Arc;
use core::task::{RawWaker, RawWakerVTable, Waker};

const VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake, drop);

/// 直接根据 Task 的指针重新构造 Waker
unsafe fn clone(p: *const ()) -> RawWaker {
    RawWaker::new(p, &VTABLE)
}

/// 根据 Waker 内部的无类型指针，得到 TaskRef，
/// Arc::from_raw 不会对引用计数自增
unsafe fn wake(p: *const ()) {
    wakeup_task(Arc::from_raw(p as *const Task))
}

/// 创建 waker 时没有增加引用计数，因此不需要实现 Drop
unsafe fn drop(_p: *const ()) {}

/// 使用 Arc::as_ptr 直接获取内部的数据指针
/// 不会对引用计数产生影响，也不会对消耗 Arc 指针
/// Waker 内部无类型擦除的指针是指向 Task，
/// 指针的生命周期与 TaskRef 的生命周期相同
/// 只要 TaskRef 没有释放，Waker 一直有效
pub fn waker_from_task(task_ref: &TaskRef) -> Waker {
    unsafe { Waker::from_raw(RawWaker::new(TaskRef::as_ptr(task_ref) as _, &VTABLE)) }
}
