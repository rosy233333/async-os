/// 位置无关的无锁侵入式链表
use crate::get_data_base;
use core::marker::PhantomData;
use core::{fmt, ptr};

/// An intrusive linked list
///
/// A clean room implementation of the one used in CS140e 2018 Winter
///
/// Thanks Sergio Benitez for his excellent work,
/// See [CS140e](https://cs140e.sergio.bz/) for more information
#[derive(Copy, Clone)]
pub struct LinkedList {
    /// head 的这些字段的访问可以不用手动进行修改偏移
    head: *mut usize,
}

unsafe impl Send for LinkedList {}

pub(crate) const EMPTY_FLAG: *mut usize = 0x74f as *mut usize;

impl LinkedList {
    /// Create a new LinkedList
    pub const fn new() -> LinkedList {
        LinkedList {
            head: unsafe { ptr::NonNull::new_unchecked(EMPTY_FLAG).as_ptr() },
        }
    }

    /// Return `true` if the list is empty
    pub fn is_empty(&self) -> bool {
        self.head == EMPTY_FLAG
    }

    /// Push `item` to the front of the list
    /// item 是相较于数据段的偏移，需要获取到实际的地址才可以进行操作
    pub unsafe fn push(&mut self, item: *mut usize) {
        *((item as usize + get_data_base()) as *mut usize) = self.head as usize;
        // *item = self.head as usize;
        self.head = item;
    }

    /// Try to remove the first item in the list
    pub fn pop(&mut self) -> Option<*mut usize> {
        match self.is_empty() {
            true => None,
            false => {
                // Advance head pointer
                let item = self.head;
                self.head =
                    unsafe { *((item as usize + get_data_base()) as *mut usize) as *mut usize };
                // self.head = unsafe { *item as *mut usize };
                Some(item)
            }
        }
    }

    /// Return an iterator over the items in the list
    pub fn iter(&self) -> Iter {
        Iter {
            curr: self.head,
            list: PhantomData,
        }
    }

    /// Return an mutable iterator over the items in the list
    /// 这里的 prev 的设置还有点问题
    pub fn iter_mut(&mut self) -> IterMut {
        IterMut {
            prev: unsafe { (&mut self.head as *mut *mut usize) as usize - get_data_base() }
                as *mut usize,
            // prev: &mut self.head as *mut *mut usize as *mut usize,
            curr: self.head,
            list: PhantomData,
        }
    }
}

impl fmt::Debug for LinkedList {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

/// An iterator over the linked list
pub struct Iter<'a> {
    curr: *mut usize,
    list: PhantomData<&'a LinkedList>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = *mut usize;

    fn next(&mut self) -> Option<Self::Item> {
        if self.curr == EMPTY_FLAG {
            None
        } else {
            let item = self.curr;
            let next = unsafe { *((item as usize + get_data_base()) as *mut usize) as *mut usize };
            // let next = unsafe { *item as *mut usize };
            self.curr = next;
            Some(item)
        }
    }
}

/// Represent a mutable node in `LinkedList`
pub struct ListNode {
    prev: *mut usize,
    curr: *mut usize,
}

impl ListNode {
    /// Remove the node from the list
    /// 不用给出实际的地址，只给出偏移量
    pub fn pop(self) -> *mut usize {
        // Skip the current one
        unsafe {
            *((self.prev as usize + get_data_base()) as *mut usize) =
                *((self.curr as usize + get_data_base()) as *mut usize);
        }
        self.curr
    }

    /// Returns the pointed address
    /// 不用给出实际的地址，只给出偏移量
    pub fn value(&self) -> *mut usize {
        self.curr
    }
}

/// A mutable iterator over the linked list
pub struct IterMut<'a> {
    list: PhantomData<&'a mut LinkedList>,
    prev: *mut usize,
    curr: *mut usize,
}

impl<'a> Iterator for IterMut<'a> {
    type Item = ListNode;

    fn next(&mut self) -> Option<Self::Item> {
        if self.curr == EMPTY_FLAG {
            None
        } else {
            let res = ListNode {
                prev: self.prev,
                curr: self.curr,
            };
            self.prev = self.curr;
            self.curr =
                unsafe { *((self.curr as usize + get_data_base()) as *mut usize) as *mut usize };
            Some(res)
        }
    }
}
