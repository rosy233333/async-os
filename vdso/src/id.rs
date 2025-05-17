/// 任务 ID

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C)]
pub struct TaskId {
    os_id: usize,
    process_id: usize,
    task_id: usize,
}

impl TaskId {
    pub const NULL: Self = Self {
        os_id: 0,
        process_id: 0,
        task_id: 0,
    };

    pub const fn is_null(&self) -> bool {
        self.os_id == 0 && self.process_id == 0 && self.task_id == 0
    }

    pub const fn new(os_id: usize, process_id: usize, task_id: usize) -> Self {
        Self {
            os_id,
            process_id,
            task_id,
        }
    }

    pub const fn os_id(&self) -> usize {
        self.os_id
    }

    pub const fn process_id(&self) -> usize {
        self.process_id
    }

    pub const fn task_id(&self) -> usize {
        self.task_id
    }

    pub const fn get_priority(&self) -> isize {
        self.task_id as isize & 0x1f
    }

    pub const fn task_ptr_value(&self) -> usize {
        self.task_id & (!0x1f)
    }
}
