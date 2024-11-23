use crate::BaseScheduler;
use alloc::sync::Arc;
use core::{cell::UnsafeCell, ops::Deref};
use taic_driver::{Taic, TaskId, TaskMeta};

/// A task wrapper for the [`TAICScheduler`].
///
/// It add a task metadata to use in Taic scheduler.
#[repr(transparent)]
pub struct TAICTask<T> {
    meta: UnsafeCell<TaskMeta<T>>,
}

impl<T> TAICTask<T> {
    /// Creates a new [`TAICTask`] from the inner task struct.
    pub const fn new(inner: T) -> Self {
        Self {
            meta: UnsafeCell::new(TaskMeta::new(0, false, inner)),
        }
    }

    /// Returns a reference to the inner task struct.
    pub const fn inner(&self) -> &T {
        self.get_meta().inner.as_ref().unwrap()
    }

    /// Get the task meta
    const fn get_meta(&self) -> &TaskMeta<T> {
        unsafe { &*self.meta.get() }
    }

    /// Get the mut task meta
    const fn get_meta_mut(&self) -> &mut TaskMeta<T> {
        unsafe { &mut *self.meta.get() }
    }
}

impl<T> Deref for TAICTask<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.get_meta().inner.as_ref().unwrap()
    }
}

unsafe impl<T> Sync for TAICTask<T> {}
unsafe impl<T> Send for TAICTask<T> {}

const PHYSICAL_OFFSET: usize = 0xffff_ffc0_0000_0000;
const TAIC_MMIO_ADDR: usize = 0x100_0000 + PHYSICAL_OFFSET;
static mut COUNT: usize = 0;

/// A Taic scheduler.
pub struct TAICScheduler<T> {
    inner: Taic,
    _phantom: core::marker::PhantomData<T>,
}

impl<T> TAICScheduler<T> {
    /// Creates a new empty [`TAICScheduler`].
    pub const fn new() -> Self {
        Self {
            inner: Taic::new(TAIC_MMIO_ADDR),
            _phantom: core::marker::PhantomData,
        }
    }
    /// get the name of scheduler
    pub fn scheduler_name() -> &'static str {
        "Taic"
    }
}

#[cfg(feature = "taic_load_balanced")]
static mut OS_TID: TaskId = TaskId::EMPTY;

impl<T> BaseScheduler for TAICScheduler<T> {
    type SchedItem = Arc<TAICTask<T>>;

    fn init(&mut self) {
        #[cfg(feature = "taic_load_balanced")]
        unsafe {
            if OS_TID.value() == 0 {
                let tid: TaskId = Arc::new(TaskMeta::<T>::init()).into();
                OS_TID = tid.phy_val(PHYSICAL_OFFSET);
            }
            self.inner = Taic::new(TAIC_MMIO_ADDR + COUNT * 0x1000);
            self.inner.switch_os::<T>(Some(OS_TID));
            COUNT += 1;
        }
        #[cfg(not(feature = "taic_load_balanced"))]
        unsafe {
            self.inner = Taic::new(TAIC_MMIO_ADDR + COUNT * 0x1000);
            let tid: TaskId = Arc::new(TaskMeta::<T>::init()).into();
            self.inner
                .switch_os::<T>(Some(tid.phy_val(PHYSICAL_OFFSET)));
            COUNT += 1;
        }
    }

    fn add_task(&mut self, task: Self::SchedItem) {
        let meta = Arc::into_raw(task) as *const TaskMeta<T>;
        let tid = TaskId::from(meta).phy_val(PHYSICAL_OFFSET);
        self.inner.add(tid);
    }

    fn remove_task(&mut self, _task: &Self::SchedItem) -> Option<Self::SchedItem> {
        unimplemented!()
    }

    fn pick_next_task(&mut self) -> Option<Self::SchedItem> {
        if let Ok(tid) = self.inner.fetch() {
            let tid = tid.virt_val(PHYSICAL_OFFSET);
            let meta: *const TaskMeta<T> = tid.into();
            return Some(unsafe { Arc::from_raw(meta as *const TAICTask<T>) });
        }
        None
    }

    fn put_prev_task(&mut self, prev: Self::SchedItem, _preempt: bool) {
        prev.get_meta_mut().is_preempt = _preempt;
        let meta = Arc::into_raw(prev) as *const TaskMeta<T>;
        let tid = TaskId::from(meta).phy_val(PHYSICAL_OFFSET);
        self.inner.add(tid)
    }

    fn task_tick(&mut self, _current: &Self::SchedItem) -> bool {
        false // no reschedule
    }

    fn set_priority(&mut self, _task: &Self::SchedItem, _prio: isize) -> bool {
        false
    }
}
