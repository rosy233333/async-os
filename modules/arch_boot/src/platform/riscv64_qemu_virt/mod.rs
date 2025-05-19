mod boot;

#[cfg(feature = "smp")]
pub mod mp;

unsafe extern "C" fn rust_entry(cpu_id: usize, dtb: usize) {
    axhal::mem::clear_bss();
    axhal::platform::time::init_board_info(dtb);
    axlog::init();
    axlog::set_max_level(option_env!("AX_LOG").unwrap_or("")); // no effect if set `log-level-*` features
    crate::vdso::init_vdso(cpu_id);
    runtime::rust_main(cpu_id, dtb);

    #[cfg(feature = "smp")]
    crate::mp::start_secondary_cpus(cpu_id);

    while !runtime::is_init_ok() {
        core::hint::spin_loop();
    }

    extern "C" {
        fn trampoline(tf: usize, has_trap: bool, from_user: bool) -> !;
    }

    unsafe {
        trampoline(0, false, false);
    }
}

#[cfg(feature = "smp")]
unsafe extern "C" fn rust_entry_secondary(cpu_id: usize) {
    trampoline::init_interrupt();
    runtime::rust_main_secondary(cpu_id);
}
