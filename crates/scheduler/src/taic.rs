use crate::BaseScheduler;
use alloc::sync::Arc;
use core::ops::Deref;
use taic_driver::{LocalQueue, Taic};

/// A task wrapper for the [`TAICScheduler`].
///
/// It add a task metadata to use in Taic scheduler.
#[repr(transparent)]
pub struct TAICTask<T> {
    inner: T,
}

impl<T> TAICTask<T> {
    /// Creates a new [`TAICTask`] from the inner task struct.
    pub const fn new(inner: T) -> Self {
        Self { inner }
    }

    /// Returns a reference to the inner task struct.
    pub const fn inner(&self) -> &T {
        &self.inner
    }
}

impl<T> Deref for TAICTask<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

unsafe impl<T> Sync for TAICTask<T> {}
unsafe impl<T> Send for TAICTask<T> {}

const TAIC_BASE: usize = axconfig::PHYS_VIRT_OFFSET + axconfig::MMIO_REGIONS[1].0;
const LQ_NUM: usize = 2;
const TAIC: Taic = Taic::new(TAIC_BASE, LQ_NUM);

/// A Taic scheduler.
pub struct TAICScheduler<T> {
    inner: LocalQueue,
    _phantom: core::marker::PhantomData<T>,
}

impl<T> TAICScheduler<T> {
    /// Creates a new empty [`TAICScheduler`].
    pub fn new() -> Self {
        Self {
            inner: TAIC.alloc_lq(1, 0).unwrap(),
            _phantom: core::marker::PhantomData,
        }
    }
    /// get the name of scheduler
    pub fn scheduler_name() -> &'static str {
        "Taic"
    }

    pub fn register_sender(&self, recv_os: usize, recv_proc: usize) {
        self.inner.register_sender(recv_os, recv_proc);
    }

    pub fn cancel_sender(&self, recv_os: usize, recv_proc: usize) {
        self.inner.cancel_sender(recv_os, recv_proc);
    }

    pub fn register_receiver(&self, send_os: usize, send_proc: usize, handler: usize) {
        self.inner.register_receiver(send_os, send_proc, handler);
    }

    pub fn send_intr(&self, recv_os: usize, recv_proc: usize) {
        self.inner.send_intr(recv_os, recv_proc);
    }

    pub fn whart(&self, hartid: usize) {
        self.inner.whart(hartid);
    }

    pub fn register_extintr(&self, irq: usize, handler: usize) {
        self.inner.register_extintr(irq, handler);
    }
}

impl<T> BaseScheduler for TAICScheduler<T> {
    type SchedItem = Arc<TAICTask<T>>;

    fn init(&mut self) {}

    fn add_task(&mut self, task: Self::SchedItem) {
        let tid = Arc::into_raw(task) as *const T as usize;
        self.inner.task_enqueue(tid);
    }

    fn remove_task(&mut self, _task: &Self::SchedItem) -> Option<Self::SchedItem> {
        unimplemented!()
    }

    fn pick_next_task(&mut self) -> Option<Self::SchedItem> {
        if let Some(tid) = self.inner.task_dequeue() {
            return Some(unsafe { Arc::from_raw(tid as *const TAICTask<T>) });
        }
        None
    }

    fn put_prev_task(&mut self, prev: Self::SchedItem, _preempt: bool) {
        self.add_task(prev);
    }

    fn task_tick(&mut self, _current: &Self::SchedItem) -> bool {
        false // no reschedule
    }

    fn set_priority(&mut self, _task: &Self::SchedItem, _prio: isize) -> bool {
        false
    }
}
