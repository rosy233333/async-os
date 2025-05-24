use alloc::sync::Arc;
use core::{cell::UnsafeCell, ops::Deref};
use syscalls::raw::syscall0;
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

const LQ_NUM: usize = 2;
const TAIC: Taic = Taic::new(0, LQ_NUM);

/// A Taic scheduler.
pub struct TAICScheduler<T> {
    inner: Option<LocalQueue>,
    _phantom: core::marker::PhantomData<T>,
}

impl<T> TAICScheduler<T> {
    /// Creates a new empty [`TAICScheduler`].
    pub fn new() -> Self {
        Self {
            inner: None,
            _phantom: core::marker::PhantomData,
        }
    }
    /// get the name of scheduler
    pub fn scheduler_name() -> &'static str {
        "Taic"
    }
}

impl<T> TAICScheduler<T> {
    pub(crate) fn init(&mut self) {
        const GET_TAIC: usize = 555;
        let taic_base = unsafe { syscall0(GET_TAIC, None) };
        println!("taic_base: {:#x}", taic_base);
        // TODO: GET a actual TAIC
        self.inner = Some(LocalQueue::new(taic_base, TAIC));
    }

    pub(crate) fn add_task(&mut self, task: Arc<TAICTask<T>>) {
        let tid = Arc::into_raw(task) as *const _ as usize;
        self.inner.as_ref().unwrap().task_enqueue(tid);
    }

    pub(crate) fn remove_task(&mut self, _task: Arc<TAICTask<T>>) -> Option<Arc<TAICTask<T>>> {
        unimplemented!()
    }

    pub(crate) fn pick_next_task(&mut self) -> Option<Arc<TAICTask<T>>> {
        if let Some(tid) = self.inner.as_ref().unwrap().task_dequeue() {
            return Some(unsafe { Arc::from_raw(tid as *const TAICTask<T>) });
        }
        None
    }

    pub(crate) fn put_prev_task(&mut self, prev: Arc<TAICTask<T>>, _preempt: bool) {
        self.add_task(prev);
    }

    pub(crate) fn task_tick(&mut self, _current: &Arc<TAICTask<T>>) -> bool {
        false // no reschedule
    }

    pub(crate) fn set_priority(&mut self, _task: &Arc<TAICTask<T>>, _prio: isize) -> bool {
        false
    }
}
