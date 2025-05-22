/// Initializes the trampoline (for the primary CPU).
pub fn init_trampoline() {
    #[cfg(feature = "irq")]
    task_api::init();
    taskctx::init_scheduler();
    #[cfg(feature = "monolithic")]
    process::init(|| alloc::boxed::Box::pin(crate::user_task_top()));
}

#[cfg(feature = "smp")]
/// Initializes the trampoline for secondary CPUs.
pub fn init_trampoline_secondary() {
    taskctx::init_scheduler();
    #[cfg(feature = "monolithic")]
    process::init_secondary();
}
