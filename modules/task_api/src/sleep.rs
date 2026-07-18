use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
#[cfg(feature = "irq")]
use kernel_guard::{BaseGuard, NoPreemptIrqSave};

#[derive(Debug)]
pub struct SleepFuture {
    #[cfg(feature = "irq")]
    _has_sleep: bool,
    // #[cfg(feature = "irq")]
    _irq_state: <NoPreemptIrqSave as BaseGuard>::State,
    deadline: axhal::time::TimeValue,
}

impl SleepFuture {
    pub fn new(
        deadline: axhal::time::TimeValue,
        _irq_state: <NoPreemptIrqSave as BaseGuard>::State,
    ) -> Self {
        // #[cfg(feature = "thread-api")]
        // return Self {
        //     #[cfg(feature = "irq")]
        //     _has_sleep: false,
        //     // #[cfg(feature = "irq")]
        //     _irq_state: Default::default(),
        //     deadline,
        // };
        // #[cfg(not(feature = "thread-api"))]
        // Self {
        //     #[cfg(feature = "irq")]
        //     _has_sleep: false,
        //     // #[cfg(feature = "irq")]
        //     _irq_state: NoPreemptIrqSave::acquire(),
        //     deadline,
        // }
        Self {
            #[cfg(feature = "irq")]
            _has_sleep: false,
            // #[cfg(feature = "irq")]
            _irq_state,
            deadline,
        }
    }
}

impl Future for SleepFuture {
    type Output = bool;
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let deadline = this.deadline;
        #[cfg(feature = "thread-api")]
        return Poll::Ready(axhal::time::current_time() >= deadline);
        #[cfg(not(feature = "thread-api"))]
        {
            #[cfg(feature = "irq")]
            if !this._has_sleep {
                this._has_sleep = true;
                crate::set_alarm_wakeup(deadline, _cx.waker().clone());
                Poll::Pending
            } else {
                // 恢复中断状态
                crate::cancel_alarm(_cx.waker());
                NoPreemptIrqSave::release(this._irq_state);
                Poll::Ready(axhal::time::current_time() >= deadline)
            }
            #[cfg(not(feature = "irq"))]
            {
                NoPreemptIrqSave::release(this._irq_state);
                axhal::time::busy_wait_until(deadline);
                Poll::Ready(true)
            }
        }
    }
}
