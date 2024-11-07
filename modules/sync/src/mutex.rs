//! 只能在 async 函数以及自定义 Poll 函数中使用的 Mutex 实现。
//!
//! 该 Mutex 以协程的方式实现，去掉了 force_lock 函数，
//! 因为协作式不存在强制的释放。
//!
//! 去掉了 try_lock，因为 try_lock 本身也是一种协作的方式。
//! 当被锁上时，不等待，暂时去处理其他的事情，
//! 而这里的实现本身就是协作的方式，因此提供这个函数没有意义

use crate::WaitQueue;
use core::cell::UnsafeCell;
use core::fmt;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicUsize, Ordering};
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use task_api::current_task;

/// A mutual exclusion primitive useful for protecting shared data, similar to
/// [`std::sync::Mutex`](https://doc.rust-lang.org/std/sync/struct.Mutex.html).
///
/// When the mutex is locked, the current task will block and be put into the
/// wait queue. When the mutex is unlocked, all tasks waiting on the queue
/// will be woken up.
pub struct Mutex<T: ?Sized> {
    wq: WaitQueue,
    owner_task: AtomicUsize,
    data: UnsafeCell<T>,
}

// Same unsafe impls as `std::sync::Mutex`
unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}
unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}

/// A guard that provides mutable data access.
///
/// When the guard falls out of scope it will release the lock.
///
/// 这个数据结构可以提供同步和异步的接口，若没有使用 .await，则使用同步的接口
/// 若使用 .await，则使用异步的接口
pub struct MutexGuard<'a, T: ?Sized + 'a> {
    lock: &'a Mutex<T>,
    data: Option<*mut T>,
}

unsafe impl<'a, T: ?Sized + 'a> Send for MutexGuard<'a, T> {}

impl<T> Mutex<T> {
    /// Creates a new [`Mutex`] wrapping the supplied data.
    #[inline(always)]
    pub const fn new(data: T) -> Self {
        Self {
            wq: WaitQueue::new(),
            owner_task: AtomicUsize::new(0),
            data: UnsafeCell::new(data),
        }
    }

    /// Consumes this [`Mutex`] and unwraps the underlying data.
    #[inline(always)]
    pub fn into_inner(self) -> T {
        // We know statically that there are no outstanding references to
        // `self` so there's no need to lock.
        let Mutex { data, .. } = self;
        data.into_inner()
    }
}

impl<T: ?Sized> Mutex<T> {
    /// Returns `true` if the lock is currently held.
    ///
    /// # Safety
    ///
    /// This function provides no synchronization guarantees and so its result should be considered 'out of date'
    /// the instant it is called. Do not use it for synchronization purposes. However, it may be useful as a heuristic.
    #[inline(always)]
    pub fn is_locked(&self) -> bool {
        self.owner_task.load(Ordering::Relaxed) != 0
    }

    /// Locks the [`Mutex`] and returns a guard that permits access to the inner data.
    ///
    /// The returned value may be dereferenced for data access
    /// and the lock will be dropped when the guard falls out of scope.
    pub fn lock(&self) -> MutexGuard<T> {
        cfg_if::cfg_if! {
            if #[cfg(feature = "thread")] {
                let curr = current_task();
                let waker = curr.waker();
                let current_task = waker.data() as usize;
                loop {
                    match self.owner_task.compare_exchange_weak(
                        0,
                        current_task,
                        Ordering::Acquire,
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => break,
                        Err(owner_task) => {
                            assert_ne!(
                                owner_task, current_task,
                                "{} tried to acquire mutex it already owns.",
                                curr.id_name(),
                            );
                            self.wq.wait_until(|| !self.is_locked());
                        }
                    }
                }
                return MutexGuard {
                    lock: self,
                    data: Some(self.data.get()),
                };
            } else if #[cfg(not(feature = "thread"))] {
                return MutexGuard {
                    lock: self,
                    data: None,
                }
            }
        }
    }

    /// Try to lock this [`Mutex`], returning a lock guard if successful.
    #[inline(always)]
    pub fn try_lock(&self) -> Option<MutexGuard<T>> {
        let waker = current_task().waker();
        let current_task = waker.data() as usize;
        // The reason for using a strong compare_exchange is explained here:
        // https://github.com/Amanieu/parking_lot/pull/207#issuecomment-575869107
        if self
            .owner_task
            .compare_exchange(0, current_task, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(MutexGuard {
                lock: self,
                data: Some(self.data.get()),
            })
        } else {
            None
        }
    }

    /// Force unlock the [`Mutex`].
    ///
    /// # Safety
    ///
    /// This is *extremely* unsafe if the lock is not held by the current
    /// thread. However, this can be useful in some instances for exposing
    /// the lock to FFI that doesn’t know how to deal with RAII.
    pub unsafe fn force_unlock(&self) {
        let curr = current_task();
        let waker = curr.waker();
        let current_task = waker.data() as usize;
        let owner_task = self.owner_task.swap(0, Ordering::Release);
        assert_eq!(
            owner_task,
            current_task,
            "{} tried to release mutex it doesn't own",
            curr.id_name()
        );
        self.wq.notify_one();
    }

    /// Returns a mutable reference to the underlying data.
    ///
    /// Since this call borrows the [`Mutex`] mutably, and a mutable reference is guaranteed to be exclusive in
    /// Rust, no actual locking needs to take place -- the mutable borrow statically guarantees no locks exist. As
    /// such, this is a 'zero-cost' operation.
    #[inline(always)]
    pub fn get_mut(&mut self) -> &mut T {
        // We know statically that there are no other references to `self`, so
        // there's no need to lock the inner mutex.
        unsafe { &mut *self.data.get() }
    }
}

impl<T: ?Sized + Default> Default for Mutex<T> {
    #[inline(always)]
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<T: ?Sized + fmt::Debug> fmt::Debug for Mutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.try_lock() {
            Some(guard) => write!(f, "Mutex {{ data: ")
                .and_then(|()| (*guard).fmt(f))
                .and_then(|()| write!(f, "}}")),
            None => write!(f, "Mutex {{ <locked> }}"),
        }
    }
}

impl<'a, T: ?Sized> Deref for MutexGuard<'a, T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &T {
        // We know statically that only we are referencing data
        unsafe {
            match self.data {
                Some(data) => &*data,
                None => panic!("data is none, you should use .await to get the data"),
            }
        }
    }
}

impl<'a, T: ?Sized> DerefMut for MutexGuard<'a, T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T {
        // We know statically that only we are referencing data
        unsafe {
            match self.data {
                Some(data) => &mut *data,
                None => panic!("data is none, you should use .await to get the data"),
            }
        }
    }
}

impl<'a, T: ?Sized + fmt::Debug> fmt::Debug for MutexGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl<'a, T: ?Sized> Drop for MutexGuard<'a, T> {
    /// The dropping of the [`MutexGuard`] will release the lock it was created from.
    fn drop(&mut self) {
        if self.data.is_some() {
            unsafe { self.lock.force_unlock() }
        }
    }
}

/// 这里要实现所有权的转移，否则会导致连续两次 drop，重复释放锁
impl<'a, T: ?Sized + 'a> Future for MutexGuard<'a, T> {
    type Output = Self;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self { lock, data } = self.get_mut();
        cfg_if::cfg_if! {
            if #[cfg(feature = "thread")] {
                assert!(data.is_some());
                Poll::Ready(MutexGuard {
                    lock,
                    data: data.take(),
                })
            } else if #[cfg(not(feature = "thread"))] {
                assert!(data.is_none());
                let curr = current_task();
                let current_task = _cx.waker().data() as usize;
                loop {
                    match lock.owner_task.compare_exchange_weak(
                        0,
                        current_task,
                        Ordering::Acquire,
                        Ordering::Relaxed,
                    ) {
                        Ok(_) => {
                            return Poll::Ready(MutexGuard {
                                lock,
                                data: Some(lock.data.get()),
                            });
                        },
                        Err(owner_task) => {
                            assert_ne!(
                                owner_task, current_task,
                                "{} tried to acquire mutex it already owns.",
                                curr.id_name(),
                            );

                            // 当前线程让权，并将 cx 注册到等待队列上
                            let _ = core::task::ready!(Pin::new(&mut lock.wq.wait_until(|| !lock.is_locked())).poll(_cx));
                        }
                    }
                }
                
            }
        }
    }
}
