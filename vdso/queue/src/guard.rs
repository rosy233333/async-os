use super::atomic::Shared;

pub struct Guard;

impl Guard {
    /// If this method is called from an [`unprotected`] guard, the function will simply be
    /// executed immediately.
    #[allow(unused)]
    pub fn defer<F, R>(&self, f: F)
    where
        F: FnOnce() -> R,
        F: Send + 'static,
    {
        drop(f());
    }

    pub unsafe fn defer_unchecked<F, R>(&self, f: F)
    where
        F: FnOnce() -> R,
    {
        drop(f());
    }

    pub unsafe fn defer_destroy<T>(&self, ptr: Shared<'_, T>) {
        self.defer_unchecked(move || ptr.into_owned());
    }
}

/// Returns a reference to a dummy guard that allows unprotected access to [`Atomic`]s.
///
/// This guard should be used in special occasions only. Note that it doesn't actually keep any
/// thread pinned - it's just a fake guard that allows loading from [`Atomic`]s unsafely.
///
/// Note that calling [`defer`] with a dummy guard will not defer the function - it will just
/// execute the function immediately.
///
/// If necessary, it's possible to create more dummy guards by cloning: `unprotected().clone()`.
///
/// # Safety
///
/// Loading and dereferencing data from an [`Atomic`] using this guard is safe only if the
/// [`Atomic`] is not being concurrently modified by other threads.
///
/// # Examples
///
/// ```
/// use crossbeam_epoch::{self as epoch, Atomic};
/// use std::sync::atomic::Ordering::Relaxed;
///
/// let a = Atomic::new(7);
///
/// unsafe {
///     // Load `a` without pinning the current thread.
///     a.load(Relaxed, epoch::unprotected());
///
///     // It's possible to create more dummy guards.
///     let dummy = epoch::unprotected();
///
///     dummy.defer(move || {
///         println!("This gets executed immediately.");
///     });
///
///     // Dropping `dummy` doesn't affect the current thread - it's just a noop.
/// }
/// # unsafe { drop(a.into_owned()); } // avoid leak
/// ```
///
/// The most common use of this function is when constructing or destructing a data structure.
///
/// For example, we can use a dummy guard in the destructor of a Treiber stack because at that
/// point no other thread could concurrently modify the [`Atomic`]s we are accessing.
///
/// If we were to actually pin the current thread during destruction, that would just unnecessarily
/// delay garbage collection and incur some performance cost, so in cases like these `unprotected`
/// is very helpful.
///
/// ```
/// use crossbeam_epoch::{self as epoch, Atomic};
/// use std::mem::ManuallyDrop;
/// use std::sync::atomic::Ordering::Relaxed;
///
/// struct Stack<T> {
///     head: Atomic<Node<T>>,
/// }
///
/// struct Node<T> {
///     data: ManuallyDrop<T>,
///     next: Atomic<Node<T>>,
/// }
///
/// impl<T> Drop for Stack<T> {
///     fn drop(&mut self) {
///         unsafe {
///             // Unprotected load.
///             let mut node = self.head.load(Relaxed, epoch::unprotected());
///
///             while let Some(n) = node.as_ref() {
///                 // Unprotected load.
///                 let next = n.next.load(Relaxed, epoch::unprotected());
///
///                 // Take ownership of the node, then drop its data and deallocate it.
///                 let mut o = node.into_owned();
///                 ManuallyDrop::drop(&mut o.data);
///                 drop(o);
///
///                 node = next;
///             }
///         }
///     }
/// }
/// ```
///
/// [`Atomic`]: super::Atomic
/// [`defer`]: Guard::defer
#[inline]
pub unsafe fn unprotected() -> &'static Guard {
    // An unprotected guard is just a `Guard` with its field `local` set to null.
    // We make a newtype over `Guard` because `Guard` isn't `Sync`, so can't be directly stored in
    // a `static`
    struct GuardWrapper(Guard);
    unsafe impl Sync for GuardWrapper {}
    static UNPROTECTED: GuardWrapper = GuardWrapper(Guard);
    &UNPROTECTED.0
}
