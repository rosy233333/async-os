use executor::current_task_may_uninit;
use taskctx::{TrapFrame, TrapStatus};

#[cfg(feature = "irq")]
#[doc(cfg(feature = "irq"))]
/// Handles periodic timer ticks for the task manager.
///
/// For example, advance scheduler states, checks timed events, etc.
pub fn on_timer_tick() {
    use executor::CurrentExecutor;
    task_api::check_events();
    // warn!("on_timer_tick");
    if let Some(curr) = current_task_may_uninit() {
        if CurrentExecutor::get().task_tick(curr.as_task_ref()) {
            #[cfg(feature = "preempt")]
            curr.set_preempt_pending(true);
        }
    }
}

pub fn handle_irq(_irq_num: usize, tf: &mut TrapFrame) {
    #[cfg(feature = "irq")]
    {
        let guard = kernel_guard::NoPreempt::new();
        axhal::irq::dispatch_irq(_irq_num);
        drop(guard); // rescheduling may occur when preemption is re-enabled.
        tf.trap_status = TrapStatus::Done;

        #[cfg(feature = "preempt")]
        crate::current_check_preempt_pending(tf);
    }
}

pub async fn handle_user_irq(_irq_num: usize, tf: &mut TrapFrame) {
    #[cfg(feature = "irq")]
    {
        let guard = kernel_guard::NoPreempt::new();
        axhal::irq::dispatch_irq(_irq_num);
        drop(guard); // rescheduling may occur when preemption is re-enabled.

        tf.trap_status = TrapStatus::Done;
        #[cfg(feature = "preempt")]
        crate::current_check_user_preempt_pending(tf).await;
    }
}
