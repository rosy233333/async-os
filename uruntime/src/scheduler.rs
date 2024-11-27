use alloc::sync::Arc;
use core::{cell::UnsafeCell, ops::Deref};
use syscalls::raw::syscall0;
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

static mut OS_TID: TaskId = TaskId::EMPTY;

impl<T> TAICScheduler<T> {
    pub(crate) fn init(&mut self) {
        const GET_TAIC: usize = 555;
        let taic_base = unsafe { syscall0(GET_TAIC, None) };
        println!("taic_base: {:#x}", taic_base);
        self.inner = Taic::new(taic_base);
    }

    pub(crate) fn add_task(&mut self, task: Arc<TAICTask<T>>) {
        let meta = Arc::into_raw(task) as *const TaskMeta<T>;
        // unsafe { &mut *(meta as *mut TaskMeta<T>) }.is_preempt = true;
        let tid = TaskId::from(meta);
        self.inner.add(tid);
    }

    pub(crate) fn remove_task(&mut self, _task: Arc<TAICTask<T>>) -> Option<Arc<TAICTask<T>>> {
        unimplemented!()
    }

    pub(crate) fn pick_next_task(&mut self) -> Option<Arc<TAICTask<T>>> {
        if let Ok(tid) = self.inner.fetch() {
            let meta: *const TaskMeta<T> = tid.into();
            return Some(unsafe { Arc::from_raw(meta as *const TAICTask<T>) });
        }
        None
    }

    pub(crate) fn put_prev_task(&mut self, prev: Arc<TAICTask<T>>, _preempt: bool) {
        prev.get_meta_mut().is_preempt = _preempt;
        let meta = Arc::into_raw(prev) as *const TaskMeta<T>;
        let tid = TaskId::from(meta).phy_val(PHYSICAL_OFFSET);
        self.inner.add(tid)
    }

    pub(crate) fn task_tick(&mut self, _current: &Arc<TAICTask<T>>) -> bool {
        false // no reschedule
    }

    pub(crate) fn set_priority(&mut self, _task: &Arc<TAICTask<T>>, _prio: isize) -> bool {
        false
    }

    pub fn send(&mut self, recv_os_id: TaskId, recv_proc_id: TaskId, recv_task_id: TaskId) {
        self.inner.send_intr(recv_os_id, recv_proc_id, recv_task_id);
    }
}
