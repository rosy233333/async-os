use alloc::boxed::Box;

use crate::vsched2::init_vsched2;

/// Initializes the trampoline (for the primary CPU).
pub fn init_trampoline() {
    executor::init(|| Box::pin(crate::user_task_top()));
    // executor::init(|| Box::pin(async { 0 }));
    init_vsched2();
}

#[cfg(feature = "smp")]
/// Initializes the trampoline for secondary CPUs.
pub fn init_trampoline_secondary() {
    use crate::vsched2::init_vsched2_secondary;

    executor::init_secondary();
    init_vsched2_secondary();
}
