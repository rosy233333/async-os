use alloc::collections::btree_map::BTreeMap;
use spinlock::SpinRaw;

use crate::prio_queue::{KtaskInfo, Scheduler};

pub struct VdsoData {
    /// 存储所有调度器
    pub(crate) schedulers: SpinRaw<BTreeMap<usize, Scheduler>>,
    /// 存储用户调度器和内核任务的对应关系（用于优先级更新）
    pub(crate) uscheduler_ktask: SpinRaw<BTreeMap<usize, KtaskInfo>>,
    /// 用于GlobalAllocator进行动态内存分配的区域
    pub(crate) alloc_area: AllocArea
}

const PAGE_SIZE: usize = 0x1000;
#[repr(align(1024))]
pub struct AllocArea(pub(crate) [u8; PAGE_SIZE]);

// 对AllocArea的访问都会加锁
unsafe impl Sync for AllocArea { }
unsafe impl Send for AllocArea { }

impl VdsoData {
    pub const fn new() -> Self {
        Self {
            schedulers: SpinRaw::new(BTreeMap::new()),
            uscheduler_ktask: SpinRaw::new(BTreeMap::new()),
            alloc_area: AllocArea([0; PAGE_SIZE])
        }
    }
}