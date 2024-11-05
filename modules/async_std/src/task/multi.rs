//! Task APIs for multi-task configuration.

extern crate alloc;

use crate::io;
use alloc::{string::String, sync::Arc};
use aos_api::task::{self as api, JoinFuture, TaskHandle};
use axerrno::ax_err_type;
use core::ops::Deref;
use core::pin::Pin;
use core::task::{Context, Poll};
use core::{cell::UnsafeCell, future::Future, num::NonZeroU64};

/// A unique identifier for a running task.
#[derive(Eq, PartialEq, Clone, Copy, Debug)]
pub struct Tid(NonZeroU64);

/// A handle to a task.
pub struct Task {
    id: Tid,
}

impl Tid {
    /// This returns a numeric identifier for the task identified by this
    /// `ThreadId`.
    pub fn as_u64(&self) -> NonZeroU64 {
        self.0
    }
}

impl Task {
    fn from_id(id: u64) -> Self {
        Self {
            id: Tid(NonZeroU64::new(id).unwrap()),
        }
    }

    /// Gets the task's unique identifier.
    pub fn id(&self) -> Tid {
        self.id
    }
}

/// Thread factory, which can be used in order to configure the properties of
/// a new task.
///
/// Methods can be chained on it in order to configure it.
#[derive(Debug)]
pub struct Builder {
    // A name for the task-to-be, for identification in panic messages
    name: Option<String>,
}

impl Builder {
    /// Generates the base configuration for spawning a task, from which
    /// configuration methods can be chained.
    pub const fn new() -> Builder {
        Builder { name: None }
    }

    /// Names the task-to-be.
    pub fn name(mut self, name: String) -> Builder {
        self.name = Some(name);
        self
    }

    /// Spawns a new task by taking ownership of the `Builder`, and returns an
    /// [`io::Result`] to its [`JoinHandle`].
    ///
    /// The spawned task may outlive the caller (unless the caller task
    /// is the main task; the whole process is terminated when the main
    /// task finishes). The join handle can be used to block on
    /// termination of the spawned task.
    pub fn spawn<F, T>(self, f: F) -> io::Result<JoinHandle<T>>
    where
        F: Future<Output = T> + 'static,
        T: Send + 'static,
    {
        unsafe { self.spawn_unchecked(f) }
    }

    unsafe fn spawn_unchecked<F, T>(self, f: F) -> io::Result<JoinHandle<T>>
    where
        F: Future<Output = T> + 'static,
        T: Send + 'static,
    {
        let name = self.name.unwrap_or_default();

        let my_packet = Arc::new(Packet {
            result: UnsafeCell::new(None),
        });
        let their_packet = my_packet.clone();

        let main = async {
            let ret = f.await;
            // SAFETY: `their_packet` as been built just above and moved by the
            // closure (it is an Arc<...>) and `my_packet` will be stored in the
            // same `JoinHandle` as this closure meaning the mutation will be
            // safe (not modify it and affect a value far away).
            unsafe { *their_packet.result.get() = Some(ret) };
            drop(their_packet);
            0
        };

        let task = api::spawn(main, name);
        Ok(JoinHandle {
            task: Task::from_id(task.id()),
            native: task,
            packet: my_packet,
        })
    }
}

/// Gets a handle to the task that invokes it.
pub fn current() -> Task {
    let id = api::current_task_id();
    Task::from_id(id)
}

/// Spawns a new task, returning a [`JoinHandle`] for it.
///
/// The join handle provides a [`join`] method that can be used to join the
/// spawned task.
///
/// The default task name is an empty string. The default task stack size is
/// [`arceos_api::config::TASK_STACK_SIZE`].
///
/// [`join`]: JoinHandle::join
pub fn spawn<T, F>(f: F) -> JoinHandle<T>
where
    F: Future<Output = T> + 'static,
    T: Send + 'static,
{
    Builder::new().spawn(f).expect("failed to spawn task")
}

struct Packet<T> {
    result: UnsafeCell<Option<T>>,
}

unsafe impl<T> Sync for Packet<T> {}

/// An owned permission to join on a task (block on its termination).
///
/// A `JoinHandle` *detaches* the associated task when it is dropped, which
/// means that there is no longer any handle to the task and no way to `join`
/// on it.
pub struct JoinHandle<T> {
    native: TaskHandle,
    task: Task,
    packet: Arc<Packet<T>>,
}

unsafe impl<T> Send for JoinHandle<T> {}
unsafe impl<T> Sync for JoinHandle<T> {}

impl<T: Unpin> JoinHandle<T> {
    /// Extracts a handle to the underlying task.
    pub fn task(&self) -> &Task {
        &self.task
    }

    /// Waits for the associated task to finish.
    ///
    /// This function will return immediately if the associated task has
    /// already finished.
    #[allow(unused_mut)]
    pub fn join(mut self) -> JoinFutureHandle<T> {
        let _inner = api::wait_for_exit(self.native);
        #[cfg(feature = "thread")]
        {
            let res = _inner.map_or_else(
                || Err(ax_err_type!(BadState)),
                |_| {
                    Arc::get_mut(&mut self.packet)
                        .unwrap()
                        .result
                        .get_mut()
                        .take()
                        .ok_or_else(|| ax_err_type!(BadState))
                },
            );
            return JoinFutureHandle {
                res: Some(res),
                _inner,
                _packet: self.packet,
            };
        }
        #[cfg(not(feature = "thread"))]
        return JoinFutureHandle {
            res: None,
            _inner,
            _packet: self.packet,
        };
    }
}

pub struct JoinFutureHandle<T: Unpin> {
    res: Option<io::Result<T>>,
    _inner: JoinFuture,
    _packet: Arc<Packet<T>>,
}

impl<T: Unpin> Future for JoinFutureHandle<T> {
    type Output = io::Result<T>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self {
            res,
            _inner,
            _packet,
        } = self.get_mut();
        #[cfg(feature = "thread")]
        {
            assert!(res.is_some());
            return Poll::Ready(res.take().unwrap());
        }
        #[cfg(not(feature = "thread"))]
        {
            assert!(res.is_none());
            Pin::new(_inner).as_mut().poll(_cx).map(|res| {
                res.map_or_else(
                    || Err(ax_err_type!(BadState)),
                    |_| {
                        Arc::get_mut(_packet)
                            .unwrap()
                            .result
                            .get_mut()
                            .take()
                            .ok_or_else(|| ax_err_type!(BadState))
                    },
                )
            })
        }
    }
}

impl<T: Unpin> Deref for JoinFutureHandle<T> {
    type Target = io::Result<T>;

    fn deref(&self) -> &Self::Target {
        &self.res.as_ref().unwrap()
    }
}
