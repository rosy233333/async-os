use core::{mem::ManuallyDrop, ops::Deref, task::Waker};

use alloc::sync::Arc;
use spinlock::SpinNoIrq;

use crate::{Scheduler, Task, TaskRef};

fn current_task_ptr() -> *const super::Task {
    vdso::current_task().task_ptr_value() as _
}

fn set_current_task_ptr(ptr: *const super::Task) {
    if ptr.is_null() {
        vdso::set_current_task(vdso::TaskId::NULL);
    } else {
        let task = unsafe { &*(ptr as *const super::Task) };
        let os_id = task.get_os_id() as _;
        let process_id = task.get_process_id() as _;
        vdso::set_current_task(vdso::TaskId::new(os_id, process_id, ptr as _));
    }
}

/// A wrapper of [`TaskRef`] as the current task.
pub struct CurrentTask(ManuallyDrop<TaskRef>);

impl CurrentTask {
    pub fn try_get() -> Option<Self> {
        let ptr: *const super::Task = current_task_ptr();
        if !ptr.is_null() {
            Some(Self(unsafe { ManuallyDrop::new(TaskRef::from_raw(ptr)) }))
        } else {
            None
        }
    }

    pub fn get() -> Self {
        Self::try_get().expect("current task is uninitialized")
    }

    /// Converts [`CurrentTask`] to [`TaskRef`].
    pub fn as_task_ref(&self) -> &TaskRef {
        &self.0
    }

    pub fn clone(&self) -> TaskRef {
        self.0.deref().clone()
    }

    pub fn ptr_eq(&self, other: &TaskRef) -> bool {
        Arc::ptr_eq(&self.0, other)
    }

    pub unsafe fn init_current(init_task: TaskRef) {
        init_task.set_state(crate::TaskState::Running);
        let ptr = Arc::into_raw(init_task);
        set_current_task_ptr(ptr);
    }

    pub fn clean_current() {
        let curr = Self::get();
        let Self(arc) = curr;
        ManuallyDrop::into_inner(arc); // `call Arc::drop()` to decrease prev task reference count.
        set_current_task_ptr(0 as *const Task);
    }

    pub fn clean_current_without_drop() -> *const super::Task {
        let ptr: *const super::Task = current_task_ptr();
        set_current_task_ptr(0 as *const Task);
        ptr
    }

    pub fn waker(&self) -> Waker {
        crate::waker::waker_from_task(current_task_ptr() as _)
    }
}

impl Deref for CurrentTask {
    type Target = Task;
    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

/// A wrapper of [`Arc<SpinNoIrq<Scheduler>>`] as the PerCPU scheduler.
/// 无论是使用什么方式，都需要获取到当前 CPU 对应的调度器，在用户态同样存在这个接口
// 但由于需要与原来的方式进行对比，因此，不能直接在 vdso 模块中直接定义，需要在 vdso 中定义好基础的接口
// 在内核和用户态中进行封装
pub struct CurrentScheduler(ManuallyDrop<Arc<SpinNoIrq<Scheduler>>>);

impl CurrentScheduler {
    pub fn try_get() -> Option<Self> {
        let ptr: *const SpinNoIrq<Scheduler> = vdso::get_scheduler_ptr() as _;
        if !ptr.is_null() {
            Some(Self(unsafe { ManuallyDrop::new(Arc::from_raw(ptr)) }))
        } else {
            None
        }
    }

    pub fn get() -> Self {
        Self::try_get().expect("current scheduler is uninitialized")
    }

    /// Converts [`CurrentTask`] to [`TaskRef`].
    pub fn as_ref(&self) -> &Arc<SpinNoIrq<Scheduler>> {
        &self.0
    }

    pub fn clone(&self) -> Arc<SpinNoIrq<Scheduler>> {
        self.0.deref().clone()
    }

    pub fn ptr_eq(&self, other: &Arc<SpinNoIrq<Scheduler>>) -> bool {
        Arc::ptr_eq(&self.0, other)
    }

    pub unsafe fn init_scheduler(scheduler: Arc<SpinNoIrq<Scheduler>>) {
        let ptr = Arc::into_raw(scheduler);
        vdso::set_scheduler_ptr(ptr as _);
    }
}

impl Deref for CurrentScheduler {
    type Target = SpinNoIrq<Scheduler>;
    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}
