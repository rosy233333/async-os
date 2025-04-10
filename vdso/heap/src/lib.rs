//! 这个库是直接参考 rcore-os 组织下的 [buddy_system_allocator](https://github.com/rcore-os/buddy_system_allocator)
//! 实现的位置无关的堆分配器
//! 用在 vDSO 中
//! 测试用例在 tests 目录中，进行了简单的测试
//! 后续的改进方向可以将这个库实现为无锁的方式？
//! 目前是使用了自旋锁
//! 因此，需要保证在中断处理例程中不会使用堆分配器，直接使用预先分配的即可
#![cfg_attr(not(test), no_std)]

mod imp;
mod linked_list;
pub use imp::{Heap, LockedHeap, LockedHeapWithRescue};

extern "C" {
    fn get_data_base() -> usize;
}

#[cfg(test)]
mod tests;
