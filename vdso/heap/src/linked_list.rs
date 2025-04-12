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
        *((item as usize + get_data_base()) as *mut usize) = self.head as usize; // 读
        // *item = self.head as usize;
        self.head = item; // 写（没有验证这次写和上次读的一致性，应该改成CAS操作？）
    }

    /// Try to remove the first item in the list
    pub fn pop(&mut self) -> Option<*mut usize> {
        match self.is_empty() { // 读
            true => None,
            false => {
                // Advance head pointer
                let item = self.head; // 读
                self.head =
                    unsafe { *((item as usize + get_data_base()) as *mut usize) as *mut usize }; // 写（没有验证这次写和上次读的一致性，应该改成CAS操作？）
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
                as *mut usize, // 我觉得这个设置没问题啊
            // prev: &mut self.head as *mut *mut usize as *mut usize,
            curr: self.head,
            list: PhantomData,
        } // 该函数中虽然进行了两次对`self`的读取，但其中的`&mut self.head`在函数执行过程中是不变的，因此不涉及同步问题？
    }
}

impl fmt::Debug for LinkedList {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

/// An iterator over the linked list
/// Iter自身没有同步问题（不能共享），因此看其访问的节点是否一致即可
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
            let item = self.curr; // 获得某节点指针
            let next = unsafe { *((item as usize + get_data_base()) as *mut usize) as *mut usize }; // 访问某节点内容，因为只有一次读操作，因此没有同步问题
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

/// 虽然对IterMut有所有权约束保证唯一，但可以从IterMut中取出几个ListNode再同时操作，因此ListNode没有唯一性，需要考虑同步问题。
/// 甚至，在使用ListNode操作前，还需要检查prev是否依然指向curr。
impl ListNode {
    /// Remove the node from the list
    /// 不用给出实际的地址，只给出偏移量
    pub fn pop(self) -> *mut usize {
        // Skip the current one
        // 这句先读了本节点，再写了上一节点。需要考虑同步问题。
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

// 同样，对IterMut也不需考虑自身字段的同步问题。
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
                unsafe { *((self.curr as usize + get_data_base()) as *mut usize) as *mut usize }; // 只有一次读操作，因此没有同步问题
            Some(res)
        }
    }
}
