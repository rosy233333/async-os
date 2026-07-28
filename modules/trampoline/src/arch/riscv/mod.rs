use crate::trampoline;
use libvsched2::current_task_ptr;
use riscv::register::{
    scause::{Interrupt::SupervisorExternal, Trap},
    sie, sstatus, stvec,
};
use task_api::current_task;
use taskctx::TrapFrame;

/// Writes Supervisor Trap Vector Base Address Register (`stvec`).
#[inline]
pub fn set_trap_vector_base(stvec: usize) {
    unsafe { stvec::write(stvec, stvec::TrapMode::Direct) }
}

/// To initialize the trap vector base address.
pub fn init_interrupt() {
    // axhal::arch::disable_irqs();
    set_trap_vector_base(trap_vector_base as usize);
}

#[unsafe(naked)]
#[link_section = ".text"]
// #[repr(align(4))]
#[rustc_align(4)]
pub unsafe extern "C" fn trap_vector_base() {
    core::arch::naked_asm!(
        "
        .include \"macros_rv64.S\"
        // j       .        
        csrrw   sp, sscratch, sp            // 交换 sp 以及 sscratch 寄存器
        addi    sp, sp, -{trapframe_size}   // 在当前的内核栈上预留出 TrapFrame 的空间
        STR     a7, sp, 16
        addi    a7, a7, -1024
        bnez    a7, slow_path
        csrr    a7, scause
        addi    a7, a7, -8
        bnez    a7, slow_path               // 判断 scause == 8 && a7 == 1024 （是系统调用，且为留给调度器陷入内核的系统调用号1024），若是则不需保存上下文直接跳转raw_trap_entry，否则先保存上下文再跳转 

        j       {fast_path_entry}

        slow_path: 
        LDR     a7, sp, 16                  // 恢复a7的值
        SAVE_REGS                           // 保存通用寄存器、sepc、sstatus、sp、fs0、fs1
        mv      a0, sp
        j       {slow_path_entry}
        ",
        trapframe_size = const core::mem::size_of::<TrapFrame>(),
        fast_path_entry = sym fast_path_entry,
        slow_path_entry = sym slow_path_entry,
    )
}

fn fast_path_entry() -> ! {
    // #[cfg(any(feature = "thread-api", feature = "preempt"))]
    // {
    //     let raw_trap_entry = unsafe {
    //         &*(libvsched2::VDSO_VTABLE.raw_trap_entry.as_ref().unwrap() as *const _ as *const ()
    //             as *const fn(usize, usize) -> !)
    //     };
    //     raw_trap_entry(2, 0);
    // }
    // #[cfg(not(any(feature = "thread-api", feature = "preempt")))]
    // panic!("`thread` or `preempt` feature is not enabled!");
    // info!("trap into fast_path_entry.",);

    // 因为任务调度的接口，会在时钟中断处理前打开中断，而此时`sip.STIP`位还未清除，因此需要使用以下方法清除该位。
    axhal::time::set_oneshot_timer(u64::MAX);
    let raw_trap_entry = unsafe {
        &*(libvsched2::VDSO_VTABLE.raw_trap_entry.as_ref().unwrap() as *const _ as *const ()
            as *const fn(usize, usize) -> !)
    };
    raw_trap_entry(2, 0);
}

fn slow_path_entry(tf: &TrapFrame) -> ! {
    // #[cfg(any(feature = "thread-api", feature = "preempt"))]
    // {
    //     use alloc::boxed::Box;

    //     let tf_c = Box::new(tf.clone()); // 需要clone的原因是当前trapframe存储于内核栈上，该内核栈在出调度器时就会被回收。
    //                                      // 任务释放时，`tf_c`释放不掉，会内存泄漏。先这么实现吧。
    //     current_task().set_stack_ctx(Box::into_raw(tf_c), taskctx::CtxType::Interrupt);
    //     let raw_trap_entry = unsafe {
    //         &*(libvsched2::VDSO_VTABLE.raw_trap_entry.as_ref().unwrap() as *const _ as *const ()
    //             as *const fn(usize, usize) -> !)
    //     };
    //     // 判断是否为外部中断
    //     if tf.get_scause_type() == Trap::Interrupt(SupervisorExternal) {
    //         raw_trap_entry(1, 0);
    //     } else {
    //         raw_trap_entry(0, 0);
    //     }
    // }
    // #[cfg(not(any(feature = "thread-api", feature = "preempt")))]
    // panic!("`thread` or `preempt` feature is not enabled!");
    use alloc::boxed::Box;

    // warn!(
    //     "trap into slow_path_entry, scause: {:?}, stval: {:#x}, sepc: {:#x}, trap_stack_base: {:#x}, current_task: {:#x}",
    //     tf.get_scause_type(),
    //     tf.stval,
    //     tf.sepc,
    //     tf as *const _ as usize + core::mem::size_of::<TrapFrame>(),
    //     current_task_ptr() as usize
    // );

    // 因为任务调度的接口，会在时钟中断处理前打开中断，而此时`sip.STIP`位还未清除，因此需要使用以下方法清除该位。
    axhal::time::set_oneshot_timer(u64::MAX);
    // let sip = riscv::register::sip::read();
    // log::info!(
    //     "slow_path_entry: \nsip: timer: {}, software: {}, external: {}",
    //     sip.stimer(),
    //     sip.ssoft(),
    //     sip.sext()
    // );
    if let Trap::Exception(e) = tf.get_scause_type() {
        panic!(
            "slow_path_entry: exception: {:?}, stval: {:#x}, sepc: {:#x}",
            e, tf.stval, tf.sepc
        );
    }
    let tf_c = Box::new(tf.clone()); // 需要clone的原因是当前trapframe存储于内核栈上，该内核栈在出调度器时就会被回收。
                                     // TODO: 任务释放时，`tf_c`释放不掉，会内存泄漏。先这么实现吧。

    // warn!("slow_path_entry: before setting stack ctx");
    current_task().set_stack_ctx(Box::into_raw(tf_c), taskctx::CtxType::Interrupt);
    // warn!("slow_path_entry: after setting stack ctx");
    let raw_trap_entry = unsafe {
        &*(libvsched2::VDSO_VTABLE.raw_trap_entry.as_ref().unwrap() as *const _ as *const ()
            as *const fn(usize, usize) -> !)
    };
    // 判断是否为外部中断
    if let Trap::Interrupt(_) = tf.get_scause_type() {
        raw_trap_entry(1, 0);
    } else {
        raw_trap_entry(0, 0);
    }
}

// macro_rules! include_save_regs_macros {
//     () => {
//         core::arch::global_asm!(
//             r"
//             .macro SAVE_REGS
//                 PUSH_GENERAL_REGS                   // 保存通用寄存器

//                 csrr    t0, sepc
//                 csrr    t1, sstatus
//                 csrrw   t2, sscratch, zero          // save sscratch (sp) and zero it
//                 STR     t0, sp, 31                  // tf.sepc
//                 STR     t1, sp, 32                  // tf.sstatus
//                 STR     t2, sp, 1                   // tf.regs.sp
//                 .short  0xa622                      // fsd fs0,264(sp)
//                 .short  0xaa26                      // fsd fs1,272(sp)
//                 csrr    t0, scause
//                 csrr    t1, stval
//                 STR     t0, sp, 35                  // save scause
//                 STR     t1, sp, 36                  // save stval

//                 li      t0, 1
//                 STR     t0, sp, 37                  // update trap status
//             .endm
//             ",
//         );
//     };
// }

// macro_rules! include_restore_regs_macros {
//     () => {
//         core::arch::global_asm!(
//             r"
//             .macro RESTORE_REGS
//                 LDR     t0, sp, 31                  // load sepc from tf.sepc
//                 LDR     t1, sp, 32                  // load sstatus from tf.sstatus
//                 csrw    sepc, t0
//                 csrw    sstatus, t1
//                 .short  0x2432                      // fld fs0,264(sp)
//                 .short  0x24d2                      // fld fs1,272(sp)
//                 POP_GENERAL_REGS                    // 恢复通用寄存器
//                 LDR     sp, sp, 1                   // load sp from tf.regs.sp
//             .endm
//             ",
//         );
//     };
// }

// include_save_regs_macros!();
// include_restore_regs_macros!();

// macro_rules! include_asm_marcos {
//     () => {
//         #[cfg(target_arch = "riscv32")]
//         core::arch::global_asm!(
//             r"
//         .ifndef XLENB
//         .equ XLENB, 4

//         .macro LDR rd, rs, off
//             lw \rd, \off*XLENB(\rs)
//         .endm
//         .macro STR rs2, rs1, off
//             sw \rs2, \off*XLENB(\rs1)
//         .endm

//         .endif"
//         );

//         #[cfg(target_arch = "riscv64")]
//         core::arch::global_asm!(
//             r"
//         .ifndef XLENB
//         .equ XLENB, 8

//         .macro LDR rd, rs, off
//             ld \rd, \off*XLENB(\rs)
//         .endm
//         .macro STR rs2, rs1, off
//             sd \rs2, \off*XLENB(\rs1)
//         .endm

//         .endif",
//         );

//         core::arch::global_asm!(
//             r"
//         .ifndef .LPUSH_POP_GENERAL_REGS
//         .equ .LPUSH_POP_GENERAL_REGS, 0

//         .macro PUSH_POP_GENERAL_REGS, op
//             \op ra, sp, 0
//             \op t0, sp, 4
//             \op t1, sp, 5
//             \op t2, sp, 6
//             \op s0, sp, 7
//             \op s1, sp, 8
//             \op a0, sp, 9
//             \op a1, sp, 10
//             \op a2, sp, 11
//             \op a3, sp, 12
//             \op a4, sp, 13
//             \op a5, sp, 14
//             \op a6, sp, 15
//             \op a7, sp, 16
//             \op s2, sp, 17
//             \op s3, sp, 18
//             \op s4, sp, 19
//             \op s5, sp, 20
//             \op s6, sp, 21
//             \op s7, sp, 22
//             \op s8, sp, 23
//             \op s9, sp, 24
//             \op s10, sp, 25
//             \op s11, sp, 26
//             \op t3, sp, 27
//             \op t4, sp, 28
//             \op t5, sp, 29
//             \op t6, sp, 30
//         .endm

//         .macro PUSH_GENERAL_REGS
//             PUSH_POP_GENERAL_REGS STR
//         .endm
//         .macro POP_GENERAL_REGS
//             PUSH_POP_GENERAL_REGS LDR
//         .endm

//         .endif"
//         );
//     };
// }

// include_asm_marcos!();
