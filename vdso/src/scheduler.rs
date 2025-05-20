use crate::TaskIdTrait;
use alloc::sync::Arc;
use scheduler::BaseScheduler;

pub struct VdsoScheduler<T> {
    _phantom: core::marker::PhantomData<T>,
}

impl<T: TaskIdTrait> VdsoScheduler<T> {
    /// Creates a new empty [`FifoScheduler`].
    pub const fn new() -> Self {
        Self {
            _phantom: core::marker::PhantomData,
        }
    }
    /// get the name of scheduler
    pub fn scheduler_name() -> &'static str {
        "VDSO FIFO Scheduler"
    }

    pub fn lock(&self) -> Self {
        Self {
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<T: TaskIdTrait> BaseScheduler for VdsoScheduler<T> {
    type SchedItem = Arc<T>;

    fn init(&mut self) {}

    fn add_task(&mut self, task: Self::SchedItem) {
        let taskid = task.build_task_id();
        crate::api::add_task(taskid);
    }

    fn remove_task(&mut self, _task: &Self::SchedItem) -> Option<Self::SchedItem> {
        unimplemented!()
    }

    fn pick_next_task(&mut self) -> Option<Self::SchedItem> {
        let taskid = crate::api::pick_next_task();
        if taskid.is_null() {
            None
        } else {
            Some(unsafe { Arc::from_raw(taskid.task_ptr_value() as *const T) })
        }
    }

    fn put_prev_task(&mut self, prev: Self::SchedItem, preempt: bool) {
        let taskid = prev.build_task_id();
        crate::api::put_prev_task(taskid, preempt);
    }

    fn task_tick(&mut self, _current: &Self::SchedItem) -> bool {
        false // no reschedule
    }

    fn set_priority(&mut self, _task: &Self::SchedItem, _prio: isize) -> bool {
        false
    }
}
