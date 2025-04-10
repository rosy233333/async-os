//! 这个模块是一个位置无关的无锁队列的实现，用在 vdso 中
//! 位置无关是指这个数据结构中的实现可以用在不同的地址空间的不同虚拟地址处
//! 参考的实现为 [crossbeam 中的 Atomic 实现](https://github.com/crossbeam-rs/crossbeam/blob/master/crossbeam-epoch/src/atomic.rs)
//! 以及[基于 crossbeam 实现的 lock_free_queue 的实现](https://github.com/maolonglong/lock_free_queue.rs/blob/main/src/lib.rs)
//! 但是，这里只能支持 MPSC 的方式，MPMC 会出现问题，因为没有 GC，所以在 pop 时会将节点直接释放掉，所以多个消费者可能会同时访问到同一个节点，导致出错
//! 测试在 tests 目录中，目前在 std 环境下测试，没有进行位置偏移，更进一步的单元测试需要配合位置无关的堆分配器进行（目前仅在内核初始化时进行测试，单元测试不完整）
#![cfg_attr(not(test), no_std)]
mod atomic;
mod guard;
mod mpmc;

pub use atomic::{Atomic, Owned, Shared};
pub use crossbeam::atomic::AtomicCell;
pub use mpmc::LockFreeQueue;

extern crate alloc;

extern "C" {
    fn get_data_base() -> usize;
}

#[cfg(test)]
mod tests;
