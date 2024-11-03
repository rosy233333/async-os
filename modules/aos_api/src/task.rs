
use core::{
    future::Future, 
    pin::Pin, 
    task::{Context, Poll},
    ops::Deref,
    time::Duration
};
use alloc::string::String;

pub use task_api::{yield_now, sleep, sleep_until, join, JoinFuture};

/// A handle to a task.
pub struct TaskHandle {
    inner: trampoline::TaskRef,
    id: u64,
}

impl TaskHandle {
    /// Returns the task ID.
    pub fn id(&self) -> u64 {
        self.id
    }
}

/// A handle to a wait queue.
///
/// A wait queue is used to store sleeping tasks waiting for a certain event
/// to happen.
pub struct WaitQueueHandle(sync::WaitQueue);

impl WaitQueueHandle {
    /// Creates a new empty wait queue.
    pub const fn new() -> Self {
        Self(sync::WaitQueue::new())
    }
}

impl Deref for WaitQueueHandle {
    type Target = sync::WaitQueue;
    fn deref(&self) -> &Self::Target { 
        &self.0
    }
}

pub fn current_task_id() -> u64 {
    trampoline::current_task().id().as_u64()
}

pub fn wait_for_exit(task: TaskHandle) -> JoinFuture {
    join(&task.inner)
}

pub fn set_current_priority(prio: isize) -> crate::AxResult {
    if trampoline::set_priority(prio) {
        Ok(())
    } else {
        axerrno::ax_err!(
            BadState,
            "ax_set_current_priority: failed to set task priority"
        )
    }
}

pub fn spawn<F>(f: F, name: alloc::string::String) -> TaskHandle
where
    F: Future<Output = i32> + 'static,
{
    let inner = trampoline::spawn_raw(move || f, name);
    TaskHandle {
        id: inner.id().as_u64(),
        inner,
    }
}

pub fn wait_queue_wake(wq: &WaitQueueHandle, count: u32) {
    if count == u32::MAX {
        wq.0.notify_all();
    } else {
        for _ in 0..count {
            wq.0.notify_one();
        }
    }
}

pub fn wait_queue_wait(
    wq: &WaitQueueHandle,
    cx: &mut Context<'_>, 
    until_condition: impl Fn() -> bool + Unpin,
    timeout: Option<Duration>,
) -> Poll<bool> {
    #[cfg(feature = "irq")]
    if let Some(dur) = timeout {
        let deadline = axhal::time::current_time() + dur;
        return Pin::new(&mut wq.0.wait_timeout_until(deadline, until_condition)).poll(cx);
    }
    if timeout.is_some() {
        axlog::warn!("wait_queue_wait: the `timeout` argument is ignored without the `irq` feature");
    }
    Pin::new(&mut wq.0.wait_until(until_condition)).poll(cx).map(|_| false)
}

