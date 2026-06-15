use kernel_guard::BaseGuard;
use taskctx::TrapFrame;

#[cfg(any(feature = "thread", feature = "preempt"))]
pub fn resched() {
    let _guard = kernel_guard::NoPreemptIrqSave::acquire();
    TrapFrame::thread_ctx(set_tf_fn as usize, CtxType::Thread);
}

#[cfg(any(feature = "thread", feature = "preempt"))]
fn set_tf_fn(tf: &mut TrapFrame, ctx_type: CtxType) {
    let curr = unsafe { &*(libvsched2::current_task_ptr() as *const taskctx::Task) };
    curr.set_stack_ctx(tf as *const _, ctx_type);
    let raw_thread_entry = unsafe { libvsched2::VDSO_VTABLE.raw_thread_entry.as_ref().unwrap() };
    raw_thread_entry();
}
