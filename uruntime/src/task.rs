use crate::Scheduler;
use alloc::{boxed::Box, collections::vec_deque::VecDeque, string::String, sync::Arc};
use core::{
    cell::UnsafeCell,
    fmt,
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicIsize, AtomicU64, Ordering},
    task::Waker,
};
use std::sync::{Mutex, MutexGuard};

/// A unique identifier for a thread.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct TaskId(u64);

static ID_COUNTER: AtomicU64 = AtomicU64::new(1);
impl TaskId {
    /// Create a new task ID.
    pub fn new() -> Self {
        Self(ID_COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    /// Convert the task ID to a `u64`.
    pub const fn as_u64(&self) -> u64 {
        self.0
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

/// The possible states of a task.
#[repr(u8)]
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[allow(missing_docs)]
pub enum TaskState {
    Running = 0,
    Runable = 1,
    Blocking = 2,
    Waked = 3,
    Blocked = 4,
    Exited = 5,
}

pub struct TaskInner {
    fut: UnsafeCell<Pin<Box<dyn Future<Output = isize> + 'static>>>,

    pub(crate) wait_wakers: UnsafeCell<VecDeque<Waker>>,
    pub(crate) scheduler: Mutex<Arc<Mutex<Scheduler>>>,

    pub(crate) id: TaskId,
    pub(crate) name: UnsafeCell<String>,
    /// Whether the task is the initial task
    ///
    /// If the task is the initial task, the kernel will terminate
    /// when the task exits.
    pub(crate) state: Mutex<TaskState>,
    exit_code: AtomicIsize,
}

unsafe impl Send for TaskInner {}
unsafe impl Sync for TaskInner {}

impl TaskInner {
    pub fn new(
        name: String,
        scheduler: Arc<Mutex<Scheduler>>,
        fut: Pin<Box<dyn Future<Output = isize> + 'static>>,
    ) -> Self {
        let t = Self {
            id: TaskId::new(),
            name: UnsafeCell::new(name),
            exit_code: AtomicIsize::new(0),
            fut: UnsafeCell::new(fut),
            wait_wakers: UnsafeCell::new(VecDeque::new()),
            scheduler: Mutex::new(scheduler),
            state: Mutex::new(TaskState::Runable),
        };
        t
    }

    /// 获取到任务的 Future
    pub fn get_fut(&self) -> &mut Pin<Box<dyn Future<Output = isize> + 'static>> {
        unsafe { &mut *self.fut.get() }
    }

    /// Gets the ID of the task.
    pub const fn id(&self) -> TaskId {
        self.id
    }

    /// Gets the name of the task.
    pub fn name(&self) -> &str {
        unsafe { (*self.name.get()).as_str() }
    }

    /// Sets the name of the task.
    pub fn set_name(&self, name: &str) {
        unsafe {
            *self.name.get() = String::from(name);
        }
    }

    /// Get a combined string of the task ID and name.
    pub fn id_name(&self) -> alloc::string::String {
        alloc::format!("Task({}, {:?})", self.id.as_u64(), self.name())
    }

    /// Get the exit code
    #[inline]
    pub fn get_exit_code(&self) -> isize {
        self.exit_code.load(Ordering::Acquire)
    }

    /// Set the task exit code
    #[inline]
    pub fn set_exit_code(&self, code: isize) {
        self.exit_code.store(code, Ordering::Release)
    }

    #[inline]
    /// set the state of the task
    pub fn state(&self) -> TaskState {
        *self.state.lock().unwrap()
    }

    #[inline]
    /// state lock manually
    pub fn state_lock_manual(&self) -> MutexGuard<TaskState> {
        self.state.lock().unwrap()
    }

    #[inline]
    /// set the state of the task
    pub fn set_state(&self, state: TaskState) {
        *self.state.lock().unwrap() = state
    }

    /// Whether the task is Exited
    #[inline]
    pub fn is_exited(&self) -> bool {
        matches!(self.state(), TaskState::Exited)
    }

    /// Whether the task is runnalbe
    #[inline]
    pub fn is_runable(&self) -> bool {
        matches!(self.state(), TaskState::Runable)
    }

    /// Whether the task is blocking
    #[inline]
    pub fn is_blocking(&self) -> bool {
        matches!(self.state(), TaskState::Blocking)
    }

    /// Whether the task is blocked
    #[inline]
    pub fn is_blocked(&self) -> bool {
        matches!(self.state(), TaskState::Blocked)
    }

    pub fn get_scheduler(&self) -> Arc<Mutex<Scheduler>> {
        self.scheduler.lock().unwrap().clone()
    }

    pub fn set_scheduler(&self, scheduler: Arc<Mutex<Scheduler>>) {
        *self.scheduler.lock().unwrap() = scheduler;
    }
}

/// Methods for task switch
impl TaskInner {
    pub fn notify_waker_for_exit(&self) {
        let wait_wakers = unsafe { &mut *self.wait_wakers.get() };
        while let Some(waker) = wait_wakers.pop_front() {
            waker.wake();
        }
    }

    pub fn join(&self, waker: Waker) {
        let task = waker.data() as *const crate::Task;
        unsafe { &*task }.set_state(TaskState::Blocking);
        let wait_wakers = unsafe { &mut *self.wait_wakers.get() };
        wait_wakers.push_back(waker);
    }
}

impl fmt::Debug for TaskInner {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TaskInner")
            .field("id", &self.id)
            .field("name", &self.name)
            .finish()
    }
}

impl Drop for TaskInner {
    fn drop(&mut self) {
        println!("drop {}", self.id_name());
    }
}
