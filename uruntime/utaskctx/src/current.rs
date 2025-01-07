use crate::{Scheduler, Task, TaskRef};
use core::cell::RefCell;
use core::mem::ManuallyDrop;
use core::ops::Deref;
use alloc::sync::Arc;
use core::task::Waker;
// use std::thread_local;

// thread_local! {
//     pub static CURRENT_TASK: RefCell<*const Task> = RefCell::new(0 as *const _);
// }

pub static CURRENT_TASK: Option<CurrentTask> = None;

/// A wrapper of [`TaskRef`] as the current task.
pub struct CurrentTask(ManuallyDrop<TaskRef>);

impl CurrentTask {
    pub fn try_get() -> Option<Self> {
        // let p = CURRENT_TASK.0;
        // let ptr = CURRENT_TASK.0.with(|task| *task.borrow());
        // if !ptr.is_null() {
        //     Some(Self(unsafe { ManuallyDrop::new(TaskRef::from_raw(ptr)) }))
        // } else {
        //     None
        // }
        todo!("")
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

    pub fn init_current(init_task: TaskRef) {
        // init_task.set_state(crate::TaskState::Running);
        // let task_ptr = Arc::into_raw(init_task);
        // CURRENT_TASK.0.with(|task| task.replace(task_ptr));
        todo!("")
    }

    pub fn clean_current() {
        // let curr = Self::get();
        // let Self(arc) = curr;
        // ManuallyDrop::into_inner(arc);
        // CURRENT_TASK.0.with(|task| task.replace(0 as *const _));
        todo!("")
    }

    pub fn clean_current_without_drop() {
        // CURRENT_TASK.0.with(|task| task.replace(0 as *const _));
        todo!("")
    }

    pub fn waker(&self) -> Waker {
        let raw_task_ptr = Arc::as_ptr(&self.0);
        crate::waker::waker_from_task(raw_task_ptr)
    }
}

impl Deref for CurrentTask {
    type Target = Task;
    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}
