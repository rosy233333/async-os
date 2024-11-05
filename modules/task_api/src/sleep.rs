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
    has_sleep: bool,
    #[cfg(feature = "irq")]
    irq_state: <NoPreemptIrqSave as BaseGuard>::State,
    deadline: axhal::time::TimeValue,
}

impl SleepFuture {
    pub fn new(deadline: axhal::time::TimeValue) -> Self {
        #[cfg(feature = "thread")]
        return Self {
            #[cfg(feature = "irq")]
            has_sleep: false,
            #[cfg(feature = "irq")]
            irq_state: (),
            deadline,
        };
        #[cfg(not(feature = "thread"))]
        Self {
            #[cfg(feature = "irq")]
            has_sleep: false,
            #[cfg(feature = "irq")]
            irq_state: NoPreemptIrqSave::acquire(),
            deadline,
        }
    }
}

impl Future for SleepFuture {
    type Output = bool;
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let deadline = self.deadline;
        #[cfg(feature = "thread")]
        return Poll::Ready(axhal::time::current_time() >= deadline);
        #[cfg(not(feature = "thread"))]
        {
            #[cfg(feature = "irq")]
            if !self.has_sleep {
                self.get_mut().has_sleep = true;
                crate::set_alarm_wakeup(deadline, _cx.waker().clone());
                Poll::Pending
            } else {
                // 恢复中断状态
                NoPreemptIrqSave::release(self.irq_state);
                crate::cancel_alarm(_cx.waker());
                Poll::Ready(axhal::time::current_time() >= deadline)
            }
            #[cfg(not(feature = "irq"))]
            {
                axhal::time::busy_wait_until(deadline);
                Poll::Ready(true)
            }
        }
    }
}
