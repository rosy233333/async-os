use crate::get_data_base;

use super::atomic::{Atomic, Owned, Shared};
use super::guard::unprotected;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicUsize, Ordering};
type Link<T> = Atomic<Node<T>>;

struct Node<T> {
    elem: MaybeUninit<T>,
    next: Link<T>,
}

impl<T> Default for Node<T> {
    fn default() -> Self {
        Self::dummy()
    }
}

impl<T> Node<T> {
    fn new(elem: T) -> Self {
        Node {
            elem: MaybeUninit::new(elem),
            next: Atomic::null(),
        }
    }

    fn dummy() -> Self {
        Node {
            elem: MaybeUninit::uninit(),
            next: Atomic::null(),
        }
    }
}

#[derive(Debug)]
#[repr(C, align(64))]
pub struct LockFreeQueue<T> {
    head: Link<T>,
    tail: Link<T>,
    len: AtomicUsize,
}

unsafe impl<T: Send> Send for LockFreeQueue<T> {}
unsafe impl<T: Send> Sync for LockFreeQueue<T> {}

impl<T> Default for LockFreeQueue<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> LockFreeQueue<T> {
    pub fn new() -> Self {
        let head = Atomic::new(Node::dummy());
        let tail = head.clone();
        LockFreeQueue {
            head,
            tail,
            len: AtomicUsize::new(0),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn len(&self) -> usize {
        self.len.load(Ordering::SeqCst)
    }

    pub fn push(&self, elem: T) {
        let guard = unsafe { unprotected() };
        let new_node = Owned::new(Node::new(elem)).into_shared(guard);
        loop {
            let tail = self.tail.load(Ordering::Acquire, guard);
            // 这里要注意偏移
            let tail_next_ref = unsafe {
                let tail_next_offset = &(*tail.as_raw()).next as *const _ as usize;
                let tail_next_base = tail_next_offset + get_data_base();
                &*(tail_next_base as *mut Link<T>)
            };
            let tail_next_shared = tail_next_ref.load(Ordering::Acquire, guard);
            if tail == self.tail.load(Ordering::Acquire, guard) {
                if tail_next_shared.is_null() {
                    if tail_next_ref
                        .compare_exchange(
                            Shared::null(),
                            new_node,
                            Ordering::Release,
                            Ordering::Relaxed,
                            guard,
                        )
                        .is_ok()
                    {
                        let _ = self.tail.compare_exchange(
                            tail,
                            new_node,
                            Ordering::Release,
                            Ordering::Relaxed,
                            guard,
                        );
                        self.len.fetch_add(1, Ordering::SeqCst);
                        return;
                    }
                } else {
                    let _ = self.tail.compare_exchange(
                        tail,
                        tail_next_shared,
                        Ordering::Release,
                        Ordering::Relaxed,
                        guard,
                    );
                }
            }
        }
    }

    pub fn pop(&self) -> Option<T> {
        let guard = unsafe { unprotected() };
        loop {
            // 这里需要注意，head 和 tail 可能为 0
            let head = self.head.load(Ordering::Acquire, guard);
            let tail = self.tail.load(Ordering::Acquire, guard);
            if head.is_null() {
                return None;
            }
            // 这里也要注意修改偏移
            let head_next_ref = unsafe {
                let head_next_offset = &(*head.as_raw()).next as *const _ as usize;
                let head_next_base = head_next_offset + get_data_base();
                &*(head_next_base as *mut Link<T>)
            };
            let head_next = head_next_ref.load(Ordering::Acquire, guard);
            if head == self.head.load(Ordering::Acquire, guard) {
                if head == tail {
                    if head_next.is_null() {
                        return None;
                    }
                    let _ = self.tail.compare_exchange(
                        tail,
                        head_next,
                        Ordering::Release,
                        Ordering::Relaxed,
                        guard,
                    );
                } else if self
                    .head
                    .compare_exchange(head, head_next, Ordering::Release, Ordering::Relaxed, guard)
                    .is_ok()
                {
                    // 这里也要注意偏移
                    let elem = unsafe {
                        let head_next_base = head_next.as_raw() as usize + get_data_base();
                        let elem = (*(head_next_base as *mut Node<T>)).elem.assume_init_read();
                        // 这里不注释掉会导致无法使用 MPMC，但是可以使用 MPSC
                        // 因为这里使用 guard 是直接释放掉内存的
                        // 一旦注释，则会导致内存泄露
                        // 这里还需要进一步参考 crossbeam 的实现
                        guard.defer_destroy(head);
                        elem
                    };
                    let _ = self.len.fetch_sub(1, Ordering::SeqCst);
                    return Some(elem);
                }
            }
        }
    }
}

impl<T> Drop for LockFreeQueue<T> {
    fn drop(&mut self) {
        while self.pop().is_some() {}
        let guard = unsafe { unprotected() };
        let h = self.head.load_consume(guard);
        if h.is_null() {
            return;
        }
        unsafe {
            guard.defer_destroy(h);
        }
    }
}
