use alloc::vec::Vec;
use crate::TaskId;
pub const MAX_EXT_IRQ: usize = 0x10;

/// The Capability
#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Capability {
    /// 
    pub task_id: TaskId,
    /// 
    pub target_os_id: TaskId,
    /// 
    pub target_proc_id: TaskId,
    /// 
    pub target_task_id: TaskId,
}

impl Capability {
    /// 
    pub const EMPTY: Self = Self {
        task_id: TaskId::EMPTY,
        target_os_id: TaskId::EMPTY,
        target_proc_id: TaskId::EMPTY,
        target_task_id: TaskId::EMPTY,
    };

    /// 
    pub fn is_device_cap(&self) -> bool {
        self.task_id != TaskId::EMPTY && self.target_os_id == TaskId::EMPTY && self.target_proc_id == TaskId::EMPTY
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct DeviceCapTable(pub (crate)[Capability; MAX_EXT_IRQ]);

impl DeviceCapTable {
    /// 
    pub const EMPTY: Self = Self([Capability::EMPTY; MAX_EXT_IRQ]);

    /// 
    pub fn iter(&self) -> Vec<TaskId> {
        self.0.iter().filter(|c| c.is_device_cap()).map(|c| {
            c.task_id
        }).collect()
    }
}

// The Capability Queue
#[repr(C)]
#[derive(Debug)]
pub struct CapQueue {
    pub inner: Vec<Capability>,
    pub online: bool,
    pub count: usize,
}

impl CapQueue {
    /// 
    pub const EMPTY: Self = Self {
        inner: Vec::new(),
        online: false,
        count: 0
    };
}