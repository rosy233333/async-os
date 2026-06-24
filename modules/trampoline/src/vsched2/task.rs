use kernel_guard::BaseGuard;
use taskctx::TrapFrame;

#[cfg(any(feature = "thread", feature = "preempt"))]
pub fn resched() {
    log::info!("call resched()");
    let _guard = kernel_guard::NoPreemptIrqSave::acquire();
    log::info!("after acquire guard");
    TrapFrame::thread_ctx(set_tf_fn as usize, taskctx::CtxType::Thread);
}

#[cfg(any(feature = "thread", feature = "preempt"))]
fn set_tf_fn(tf: &mut TrapFrame, ctx_type: taskctx::CtxType) {
    log::info!("set_tf_fn: {:#x}", tf as *mut _ as usize);
    tf.trap_status = taskctx::TrapStatus::Done;
    log::info!("{:#x?}", tf);
    let curr = unsafe { &*(libvsched2::current_task_ptr() as *const taskctx::Task) };
    log::info!("after get current task");
    curr.set_stack_ctx(tf as *const _, ctx_type);
    // log::info!("tf in task:");
    // log::info!("{:#x?}", unsafe {
    //     *curr.get_stack_ctx().unwrap().trap_frame
    // });
    // loop {}
    let raw_thread_entry = unsafe { libvsched2::VDSO_VTABLE.raw_thread_entry.as_ref().unwrap() };
    log::info!(
        "into raw_thread_entry: 0x{:#x}",
        raw_thread_entry as *const _ as usize
    );
    raw_thread_entry();
}
