use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

use kernel_guard::{BaseGuard, NoPreemptIrqSave};

#[derive(Debug)]
pub struct YieldFuture {
    _has_polled: bool,
    _irq_state: <NoPreemptIrqSave as BaseGuard>::State,
}

impl YieldFuture {
    /// 关中断需要在设置任务状态前进行，因此在new函数外部完成。
    ///
    /// 协程：关中断操作获取到的状态传入new函数中。
    /// 线程：new函数直接传入default就绪。
    pub fn new(_irq_state: <NoPreemptIrqSave as BaseGuard>::State) -> Self {
        // // 这里获取中断状态，并且关中断
        // #[cfg(feature = "thread-api")]
        // let _irq_state = Default::default();
        // #[cfg(not(feature = "thread-api"))]
        // let _irq_state = NoPreemptIrqSave::acquire();
        Self {
            _has_polled: false,
            _irq_state,
        }
    }
}

impl Future for YieldFuture {
    type Output = ();
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        #[cfg(feature = "thread-api")]
        return Poll::Ready(());
        #[cfg(not(feature = "thread-api"))]
        {
            let this = self.get_mut();
            if this._has_polled {
                // 恢复原来的中断状态
                NoPreemptIrqSave::release(this._irq_state);
                Poll::Ready(())
            } else {
                this._has_polled = true;
                Poll::Pending
            }
        }
    }
}
