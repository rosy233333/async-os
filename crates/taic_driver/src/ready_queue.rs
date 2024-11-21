use alloc::vec::Vec;

use crate::task::TaskId;

#[repr(C)]
#[derive(Debug)]
pub struct ReadyQueue {
    pub inner: Vec<TaskId>,
    pub online: bool,
    pub count: usize,
}


impl ReadyQueue {
    /// 
    pub const EMPTY: Self = Self {
        inner: Vec::new(),
        online: false,
        count: 0
    };
}