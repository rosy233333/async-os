mod boot;

#[cfg(feature = "smp")]
pub mod mp;

#[no_mangle]
pub unsafe extern "C" fn rust_entry(cpu_id: usize, dtb: usize) {
    axhal::mem::clear_bss();
    axhal::cpu::init_primary(cpu_id);
    axhal::platform::time::init_board_info(dtb);
    // trampoline::init_interrupt();
    axlog::init();
    axlog::set_max_level(option_env!("AX_LOG").unwrap_or("")); // no effect if set `log-level-*` features

    runtime::rust_main(cpu_id, dtb);

    #[cfg(feature = "smp")]
    crate::mp::start_secondary_cpus(cpu_id);

    while !runtime::is_init_ok() {
        core::hint::spin_loop();
    }

    loop {}
    // trampoline::thread_exit();
}

#[cfg(feature = "smp")]
#[no_mangle]
unsafe extern "C" fn rust_entry_secondary(cpu_id: usize) {
    trampoline::init_interrupt();
    axhal::cpu::init_secondary(cpu_id);
    runtime::rust_main_secondary(cpu_id);
}
