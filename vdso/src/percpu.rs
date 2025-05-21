use queue::{AtomicCell, LockFreeQueue};

use crate::{stack_pool::StackPool, TaskId};

#[repr(C, align(64))]
pub struct PerCPU {
    /// Processor ready_queue
    ready_queue: LockFreeQueue<TaskId>,
    /// 记录的当前任务标识
    current_task: AtomicCell<Option<TaskId>>,
    /// 运行栈池
    stack_pool: StackPool,
}

const VDSO_USED_PERCPU_SIZE: usize = core::mem::size_of::<PerCPU>();

// 因为没有使用到，所以出现了问题
#[link_section = ".percpu.start"]
#[used]
static mut PERCPU: [u8; VDSO_USED_PERCPU_SIZE] = [0u8; VDSO_USED_PERCPU_SIZE];
