use core::{
    future::Future,
    ops::Deref,
    pin::Pin,
    task::{Context, Poll},
};
use kernel_guard::{BaseGuard, NoPreemptIrqSave};
use taskctx::TaskRef;

pub struct JoinFuture {
    _task: TaskRef,
    res: Option<i32>,
    _irq_state: <NoPreemptIrqSave as BaseGuard>::State,
}

impl JoinFuture {
    pub fn new(_task: TaskRef, res: Option<i32>) -> Self {
        let _irq_state = Default::default();
        Self {
            _task,
            res,
            _irq_state,
        }
    }
}

impl Future for JoinFuture {
    type Output = Option<i32>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        #[cfg(feature = "thread-api")]
        {
            assert!(this.res.is_some());
            return Poll::Ready(this.res.take());
        }
        #[cfg(not(feature = "thread-api"))]
        {
            use core::mem::ManuallyDrop;
            if this.res.is_none() {
                // 在将waker放入任务的waker队列期间，保持任务状态不变。
                let guard = this._task.state_lock_manual();
                if **guard == taskctx::TaskState::Exited {
                    drop(ManuallyDrop::into_inner(guard));
                    NoPreemptIrqSave::release(this._irq_state);
                    Poll::Ready(Some(this._task.get_exit_code() as i32))
                } else {
                    this._task.join(_cx.waker().clone());
                    drop(ManuallyDrop::into_inner(guard));
                    this._irq_state = NoPreemptIrqSave::acquire();
                    Poll::Pending
                }
            } else {
                NoPreemptIrqSave::release(this._irq_state);
                Poll::Ready(this.res.take())
            }
        }
    }
}

impl Deref for JoinFuture {
    type Target = Option<i32>;

    fn deref(&self) -> &Self::Target {
        &self.res
    }
}
