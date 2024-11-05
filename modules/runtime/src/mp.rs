//! Runtime main function for secondary CPUs.
use core::sync::atomic::{AtomicUsize, Ordering};

static ENTERED_CPUS: AtomicUsize = AtomicUsize::new(1);

/// The main entry point of the ArceOS runtime for secondary CPUs.
///
/// It is called from the bootstrapping code in [axhal].
#[no_mangle]
pub fn rust_main_secondary(cpu_id: usize) -> ! {
    ENTERED_CPUS.fetch_add(1, Ordering::Relaxed);

    info!("Secondary CPU {:x} started.", cpu_id);

    #[cfg(feature = "paging")]
    super::remap_kernel_memory().unwrap();

    axhal::platform_init_secondary();

    trampoline::init_trampoline_secondary();

    info!("Secondary CPU {:x} init OK.", cpu_id);
    super::INITED_CPUS.fetch_add(1, Ordering::Relaxed);

    while !super::is_init_ok() {
        core::hint::spin_loop();
    }

    #[cfg(feature = "irq")]
    axhal::arch::enable_irqs();

    #[cfg(all(feature = "tls", not(feature = "multitask")))]
    super::init_tls();

    let tf = unsafe { core::ptr::NonNull::<trampoline::TrapFrame>::dangling().as_mut() };
    trampoline::trampoline(tf, false, false);

    unreachable!()
}

/// The number of CPUs that have entered the runtime.
pub fn entered_cpus_num() -> usize {
    ENTERED_CPUS.load(Ordering::Acquire)
}
