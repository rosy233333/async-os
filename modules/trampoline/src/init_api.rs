use alloc::sync::Arc;
use spinlock::SpinNoIrq;
use taskctx::{CurrentScheduler, Scheduler};

/// Initializes the trampoline (for the primary CPU).
pub fn init_trampoline() {
    unsafe { CurrentScheduler::init_scheduler(Arc::new(SpinNoIrq::new(Scheduler::new()))) };
    #[cfg(feature = "monolithic")]
    process::init(|| alloc::boxed::Box::pin(crate::user_task_top()));
}

#[cfg(feature = "smp")]
/// Initializes the trampoline for secondary CPUs.
pub fn init_trampoline_secondary() {
    unsafe { CurrentScheduler::init_scheduler(Arc::new(SpinNoIrq::new(Scheduler::new()))) };
    process::init_secondary();
}
