//! Frame-pointer based backtrace for kernel-mode exceptions.

use axconfig::TASK_STACK_SIZE;
use taskctx::TrapFrame;

/// Max frames printed by [`dump_trap_backtrace`].
const MAX_FRAMES: usize = 64;

/// Kernel image text range symbols, defined by `linker_riscv64-qemu-virt.lds`.
extern "C" {
    fn _stext();
    fn _etext();
}

/// Prints the interrupted kernel execution flow's call chain via a
/// frame-pointer walk, then lets the caller panic.
///
/// User-mode exceptions only print the saved register context; the walk is
/// kernel-mode only. Every stack read is validated beforehand so a corrupt
/// frame chain cannot cause a nested page fault inside the panic path.
pub(crate) fn dump_trap_backtrace(tf: &TrapFrame) {
    let from_user = tf.sstatus & (1 << 8) == 0;
    log::error!(
        "[trap backtrace] scause: {:?}, stval: {:#x}, sepc: {:#x}, ra: {:#x}, sp: {:#x}, s0: {:#x}, mode: {}",
        tf.get_scause_type(),
        tf.stval,
        tf.sepc,
        tf.regs.ra,
        tf.regs.sp,
        tf.regs.s0,
        if from_user { "user" } else { "kernel" },
    );

    if from_user {
        log::error!("[trap backtrace] user-mode exception, kernel-mode backtrace not supported");
        return;
    }

    let text_start = _stext as usize;
    let text_end = _etext as usize;
    let sp0 = tf.regs.sp;
    let mut fp = tf.regs.s0;
    let mut pc = tf.sepc;
    for i in 0..MAX_FRAMES {
        if !(text_start..text_end).contains(&pc) {
            log::error!("[trap backtrace] stop: pc {:#x} is outside kernel text", pc);
            return;
        }
        log::error!("[trap backtrace] #{:02} pc = {:#x}", i, pc);

        // RISC-V psABI frame layout: `[fp - XLEN]` holds the return address and
        // `[fp - 2 * XLEN]` holds the previous frame pointer. Keep every read
        // inside the interrupted stack window `[sp0, sp0 + TASK_STACK_SIZE)`.
        if fp == 0 || fp % 16 != 0 || fp < sp0 + 16 || fp >= sp0 + TASK_STACK_SIZE {
            log::error!("[trap backtrace] stop: invalid frame pointer {:#x}", fp);
            return;
        }
        let next_fp = unsafe { *(fp as *const usize).sub(2) };
        let next_pc = unsafe { *(fp as *const usize).sub(1) };
        if next_fp <= fp {
            log::error!(
                "[trap backtrace] stop: frame pointer not increasing: {:#x} -> {:#x}",
                fp,
                next_fp
            );
            return;
        }
        fp = next_fp;
        pc = next_pc;
    }
    log::error!("[trap backtrace] stop: frame limit {} reached", MAX_FRAMES);
}
