#![no_std]
#![feature(naked_functions)]

extern crate alloc;
extern crate log;

mod arch;
mod current;
mod stat;
mod task;
mod waker;

use alloc::sync::Arc;
pub use arch::TrapFrame;
pub use arch::TrapStatus;
pub use current::CurrentTask;

pub type TaskRef = Arc<Task>;
pub use scheduler::BaseScheduler;
use spinlock::SpinNoIrq;
pub use task::{SchedPolicy, SchedStatus, TaskId, TaskInner, TaskState};

// #[cfg(feature = "thread")]
// pub use task::{CtxType, StackCtx};

cfg_if::cfg_if! {
    if #[cfg(feature = "sched_rr")] {
        const MAX_TIME_SLICE: usize = 5;
        pub type Task = scheduler::RRTask<TaskInner, MAX_TIME_SLICE>;
        pub type Scheduler = scheduler::RRScheduler<TaskInner, MAX_TIME_SLICE>;
    } else if #[cfg(feature = "sched_cfs")] {
        pub type Task = scheduler::CFSTask<TaskInner>;
        pub type Scheduler = scheduler::CFScheduler<TaskInner>;
    } else {
        // // If no scheduler features are set, use FIFO as the default.
        // pub type Task = scheduler::FifoTask<TaskInner>;
        // pub type Scheduler = scheduler::FifoScheduler<TaskInner>;

        // If no scheduler features are set, use FIFO as the default.
        use core::ops::Deref;

        pub struct TaskWrapper<T> {
            inner: T,
        }

        impl<T> TaskWrapper<T> {
            pub const fn new(inner: T) -> Self {
                Self { inner }
            }

            /// Returns a reference to the inner task struct.
            pub const fn inner(&self) -> &T {
                &self.inner
            }
        }

        impl<T> Deref for TaskWrapper<T> {
            type Target = T;
            #[inline]
            fn deref(&self) -> &Self::Target {
                &self.inner
            }
        }
        pub type Task = TaskWrapper<TaskInner>;
        impl vdso::TaskIdTrait for Task {
            fn get_os_id(self: &Arc<Self>) -> usize {
                self.inner.get_os_id() as _
            }
            fn get_process_id(self: &Arc<Self>) -> usize {
                self.inner.get_process_id() as _
            }
            fn get_task_id(self: &Arc<Self>) -> usize {
                Arc::into_raw(self.clone()) as usize
            }
            fn build_task_id(self: &Arc<Self>) -> vdso::TaskId {
                vdso::TaskId::new(
                    self.get_os_id(),
                    self.get_process_id(),
                    self.get_task_id(),
                )
            }
        }

        pub type Scheduler = vdso::VdsoScheduler<Task>;
    }
}

#[percpu::def_percpu]
static SCHEDULER: lazy_init::LazyInit<Arc<SpinNoIrq<Scheduler>>> = lazy_init::LazyInit::new();

pub fn init_scheduler() {
    let s = unsafe { SCHEDULER.current_ref_mut_raw() };
    let scheduler = Arc::new(SpinNoIrq::new(Scheduler::new()));
    scheduler.lock().init();
    s.init_by(scheduler);
}

pub fn current_scheduler() -> Arc<SpinNoIrq<Scheduler>> {
    SCHEDULER.with_current(|s| s.clone())
}

/// 这里不对任务的状态进行修改，在调用 waker.wake() 之前对任务状态进行修改
/// 这里直接使用 Arc，会存在问题，导致任务的引用计数减一，从而直接被释放掉
/// 因此使用任务的原始指针，只在确实需要唤醒时，才会拿到任务的 Arc 指针
pub fn wakeup_task(task_ptr: *const Task) {
    let task = unsafe { &*task_ptr };
    let mut state = task.state_lock_manual();
    match **state {
        // 任务正在运行，且没有让权，不必唤醒
        // 可能不止一个其他的任务在唤醒这个任务，因此被唤醒的任务可能是处于 Running 状态的
        TaskState::Running => (),
        // 任务准备让权，但没有让权，还在核上运行，但已经被其他核唤醒，此时只需要修改其状态即可
        // 后续的处理由正在核上运行的自己来决定
        TaskState::Blocking => **state = TaskState::Waked,
        // 任务不在运行，但其状态处于就绪状态，意味着任务已经在就绪队列中，不需要再向其中添加任务
        TaskState::Runable => (),
        // 任务不在运行，已经让权结束，不在核上运行，就绪队列中也不存在，需要唤醒
        // 只有处于 Blocked 状态的任务才能被唤醒，这时候才会拿到任务的 Arc 指针
        TaskState::Blocked => {
            **state = TaskState::Runable;
            let task_ref = unsafe { Arc::from_raw(task_ptr) };
            task.scheduler.lock().lock().add_task(task_ref);
        }
        TaskState::Waked => panic!("cannot wakeup Waked {}", task.id_name()),
        // 无法唤醒已经退出的任务
        TaskState::Exited => panic!("cannot wakeup Exited {}", task.id_name()),
    };
    drop(core::mem::ManuallyDrop::into_inner(state));
}

#[cfg(feature = "preempt")]
use kernel_guard::KernelGuardIf;

#[cfg(feature = "preempt")]
struct KernelGuardIfImpl;

#[cfg(feature = "preempt")]
#[crate_interface::impl_interface]
impl KernelGuardIf for KernelGuardIfImpl {
    fn enable_preempt() {
        // Your implementation here
        if let Some(curr) = CurrentTask::try_get() {
            curr.enable_preempt();
        }
    }
    fn disable_preempt() {
        // Your implementation here
        if let Some(curr) = CurrentTask::try_get() {
            curr.disable_preempt();
        }
    }
}
