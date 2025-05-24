use crate::{Task, TaskRef};
use alloc::sync::Arc;
use core::{mem::ManuallyDrop, ops::Deref, task::Waker};

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
        if !vdso::IS_VDSO_INIT_DONE.load(core::sync::atomic::Ordering::Relaxed) {
            return None;
        }
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
