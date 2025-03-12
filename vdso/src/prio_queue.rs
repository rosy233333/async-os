use alloc::collections::{btree_map::BTreeMap, vec_deque::VecDeque};
// use spin::Mutex;
use spinlock::SpinRaw;

// use crate::{get_schedulers, get_uscheduler_ktask};

const PRIO_NUM: usize = 4;
/// 存储所有调度器
// pub(crate) static SCHEDULERS: SpinRaw<BTreeMap<usize, Scheduler>> = SpinRaw::new(BTreeMap::new());
// pub(crate) static SCHEDULERS: Mutex<BTreeMap<usize, Scheduler>> = Mutex::new(BTreeMap::new());
/// 存储用户调度器和内核任务的对应关系（用于优先级更新）
// pub(crate) static USCHEDULER_KTASK: SpinRaw<BTreeMap<usize, KtaskInfo>> = SpinRaw::new(BTreeMap::new());
// pub(crate) static USCHEDULER_KTASK: Mutex<BTreeMap<usize, KtaskInfo>> = Mutex::new(BTreeMap::new());

/// 数组下标对应优先级，优先级数值越低，优先级越高。
pub(crate) struct Scheduler{
    pub(crate) queues: [VecDeque<usize>; PRIO_NUM],
    /// 当前任务（即上一个取出的任务）的优先级
    /// - 初始化时为PRIO_NUM；
    /// - 自从成功取出一次任务后，取值均在0 .. PRIO_NUM范围内。
    current_prio: usize,
    /// 调度器内任务的最高优先级
    /// - 0 .. PRIO_NUM：调度器内有相应优先级的任务
    /// - PRIO_NUM：调度器内没有任务
    pub(crate) highest_prio: usize
}

impl Scheduler {
    pub(crate) fn new() -> Self {
        Self{
            queues: [const { VecDeque::new() } ; PRIO_NUM],
            current_prio: PRIO_NUM,
            highest_prio: PRIO_NUM
        }
    }

    /// 返回值代表是否需要重新调度
    pub(crate) fn add_task(&mut self, task_ptr: usize, task_prio: usize) -> bool {
        // 更新最高优先级
        if self.highest_prio > task_prio {
            self.highest_prio = task_prio;
        }
        self.queues[task_prio].push_back(task_ptr);
        self.highest_prio < self.current_prio
    }

    pub(crate) fn pick_next_task(&mut self) -> Option<usize> {
        let mut picked_task: Option<usize> = None;
        let mut prio: usize = 0;
        loop {
            picked_task = self.queues[prio].pop_front();
            if picked_task.is_some() {
                // 更新当前优先级
                self.current_prio = prio;
                break;
            }
            prio += 1;
            if prio == PRIO_NUM {
                break;
            }
        }
        // 更新最高优先级
        loop {
            if prio == PRIO_NUM || !self.queues[prio].is_empty() {
                self.highest_prio = prio;
                break;
            }
            prio += 1;
        }
        picked_task
    }

    /// 返回值：
    /// - Ok(bool)：bool值代表是否需要重新调度
    /// - Err(())：找不到task_ptr代表的任务
    pub(crate) fn set_priority(&mut self, task_ptr: usize, task_prio: usize) -> Result<bool, ()> {
        let mut task: Option<usize> = None;
        for prio in self.highest_prio .. PRIO_NUM {
            let task_index = self.queues[prio].iter().position(|&p| { p == task_ptr });
            if let Some(task_index) = task_index {
                task = self.queues[prio].remove(task_index);
                assert!(task.is_some());
                break;
            }
        }
        if let Some(task) = task {
            assert!(task == task_ptr);
            self.queues[task_prio].push_back(task_ptr);
            // 更新最高优先级
            let highest_possible_prio = self.highest_prio.min(task_prio);
            let lowest_powwible_prio = task_prio;
            for prio in highest_possible_prio ..= lowest_powwible_prio {
                if !self.queues[prio].is_empty() {
                    self.highest_prio = prio;
                    return Ok(self.highest_prio < self.current_prio);
                }
            }
            panic!("unreachable");
        }
        else {
            Err(())
        }
    }

    pub(crate) fn clear_current(&mut self) {
        self.current_prio = PRIO_NUM;
    }
}

pub struct KtaskInfo{
    pub ktask_ptr: usize,
    pub cpu_id: usize
}

impl KtaskInfo {
    pub fn new(ktask_ptr: usize, cpu_id: usize) -> Self {
        Self { ktask_ptr, cpu_id }
    }
}

// /// 返回值代表内核是否需要重新调度
// pub(crate) fn update_ktask_priority(uscheduler_id: usize) -> bool {
//     let ktask_prio = { 
//         let schedulers = get_schedulers().lock();
//         let uschduler = schedulers.get(&uscheduler_id).unwrap();
//         uschduler.current_prio.min(uschduler.highest_prio)
//     };
//     let uscheduler_ktask = get_uscheduler_ktask().lock();
//     let ktask_info = uscheduler_ktask.get(&uscheduler_id).unwrap();
//     // 对set_priority的返回值使用unwrap_or(false)的原因：
//     // 用户调度器代表的内核任务可能不在调度器中（例如，正在运行），此时不需要触发重新调度
//     get_schedulers().lock().get_mut(&ktask_info.cpu_id).unwrap().set_priority(ktask_info.ktask_ptr, ktask_prio).unwrap_or(false)
// }

// pub(crate) fn get_uscheduler_from_task(task_ptr: usize) -> Option<usize> {
//     get_uscheduler_ktask().lock().iter().find(|&item| {
//         item.1.ktask_ptr == task_ptr
//     }).map(|(&k, &ref _v)| {
//         k
//     })
// }

// // pub(crate) fn delete_scheduler(scheduler_id: usize) -> Option<Scheduler> {
// //     get_schedulers().lock().remove(&scheduler_id)
// // }