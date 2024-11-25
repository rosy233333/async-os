use crate::{Scheduler, Task, TaskRef};
use std::cell::RefCell;
use std::mem::ManuallyDrop;
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::task::Waker;
use std::thread_local;

thread_local! {
    pub static CURRENT_TASK: RefCell<*const Task> = RefCell::new(0 as *const _);
    pub static SCHEDULER: RefCell<Arc<Mutex<Scheduler>>> = RefCell::new(Arc::new(Mutex::new(Scheduler::new())));
}

/// A wrapper of [`TaskRef`] as the current task.
pub struct CurrentTask(ManuallyDrop<TaskRef>);

impl CurrentTask {
    pub fn try_get() -> Option<Self> {
        let ptr = CURRENT_TASK.with(|task| *task.borrow());
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

    pub fn init_current(init_task: TaskRef) {
        init_task.set_state(crate::TaskState::Running);
        let task_ptr = Arc::into_raw(init_task);
        CURRENT_TASK.with(|task| task.replace(task_ptr));
    }

    pub fn clean_current() {
        let curr = Self::get();
        let Self(arc) = curr;
        ManuallyDrop::into_inner(arc);
        CURRENT_TASK.with(|task| task.replace(0 as *const _));
    }

    pub fn clean_current_without_drop() {
        CURRENT_TASK.with(|task| task.replace(0 as *const _));
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
