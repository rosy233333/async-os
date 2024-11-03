//! Thread APIs for multi-threading configuration.

extern crate alloc;

use crate::io;
use alloc::{string::String, sync::Arc};
use core::{cell::UnsafeCell, future::Future, num::NonZeroU64};
use core::task::{Poll, Context};
use core::pin::Pin;
use aos_api::task::{self as api, TaskHandle, JoinFuture};
use axerrno::ax_err_type;

/// A unique identifier for a running thread.
#[derive(Eq, PartialEq, Clone, Copy, Debug)]
pub struct ThreadId(NonZeroU64);

/// A handle to a thread.
pub struct Thread {
    id: ThreadId,
}

impl ThreadId {
    /// This returns a numeric identifier for the thread identified by this
    /// `ThreadId`.
    pub fn as_u64(&self) -> NonZeroU64 {
        self.0
    }
}

impl Thread {
    fn from_id(id: u64) -> Self {
        Self {
            id: ThreadId(NonZeroU64::new(id).unwrap()),
        }
    }

    /// Gets the thread's unique identifier.
    pub fn id(&self) -> ThreadId {
        self.id
    }
}

/// Thread factory, which can be used in order to configure the properties of
/// a new thread.
///
/// Methods can be chained on it in order to configure it.
#[derive(Debug)]
pub struct Builder {
    // A name for the thread-to-be, for identification in panic messages
    name: Option<String>,
}

impl Builder {
    /// Generates the base configuration for spawning a thread, from which
    /// configuration methods can be chained.
    pub const fn new() -> Builder {
        Builder {
            name: None,
        }
    }

    /// Names the thread-to-be.
    pub fn name(mut self, name: String) -> Builder {
        self.name = Some(name);
        self
    }

    /// Spawns a new thread by taking ownership of the `Builder`, and returns an
    /// [`io::Result`] to its [`JoinHandle`].
    ///
    /// The spawned thread may outlive the caller (unless the caller thread
    /// is the main thread; the whole process is terminated when the main
    /// thread finishes). The join handle can be used to block on
    /// termination of the spawned thread.
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
            thread: Thread::from_id(task.id()),
            native: task,
            packet: my_packet,
        })
    }
}

/// Gets a handle to the thread that invokes it.
pub fn current() -> Thread {
    let id = api::current_task_id();
    Thread::from_id(id)
}

/// Spawns a new thread, returning a [`JoinHandle`] for it.
///
/// The join handle provides a [`join`] method that can be used to join the
/// spawned thread.
///
/// The default task name is an empty string. The default thread stack size is
/// [`arceos_api::config::TASK_STACK_SIZE`].
///
/// [`join`]: JoinHandle::join
pub fn spawn<T, F>(f: F) -> JoinHandle<T>
where
    F: Future<Output = T> + 'static,
    T: Send + 'static,
{
    Builder::new().spawn(f).expect("failed to spawn thread")
}

struct Packet<T> {
    result: UnsafeCell<Option<T>>,
}

unsafe impl<T> Sync for Packet<T> {}

/// An owned permission to join on a thread (block on its termination).
///
/// A `JoinHandle` *detaches* the associated thread when it is dropped, which
/// means that there is no longer any handle to the thread and no way to `join`
/// on it.
pub struct JoinHandle<T> {
    native: TaskHandle,
    thread: Thread,
    packet: Arc<Packet<T>>,
}

unsafe impl<T> Send for JoinHandle<T> {}
unsafe impl<T> Sync for JoinHandle<T> {}

impl<T> JoinHandle<T> {
    /// Extracts a handle to the underlying thread.
    pub fn thread(&self) -> &Thread {
        &self.thread
    }

    /// Waits for the associated thread to finish.
    ///
    /// This function will return immediately if the associated thread has
    /// already finished.
    pub fn join(self) -> JoinFutureHandle<T> {
        let inner = api::wait_for_exit(self.native);
        JoinFutureHandle::new(inner, self.packet)
    }
}

pub struct JoinFutureHandle<T> {
    inner: JoinFuture,
    packet: Arc<Packet<T>>,
}

impl<T> JoinFutureHandle<T> {
    fn new(inner: JoinFuture, packet: Arc<Packet<T>>) -> Self {
        Self { inner, packet }
    }
}

impl<T> Future for JoinFutureHandle<T> {
    type Output = io::Result<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { inner, packet } = self.get_mut();
        Pin::new(inner).as_mut().poll(cx).map(|res| {
            res.map_or_else(
                || Err(ax_err_type!(BadState)), 
                |_| Arc::get_mut(packet)
                        .unwrap()
                        .result
                        .get_mut()
                        .take()
                        .ok_or_else(|| ax_err_type!(BadState))
            )
        })
    }
}