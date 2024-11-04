use core::{future::Future, ops::Deref, pin::Pin, task::{Context, Poll}};
use taskctx::TaskRef;

pub struct JoinFuture {
    _task: TaskRef,
    res: Option<i32>,
}

impl JoinFuture {
    pub fn new(_task: TaskRef, res: Option<i32>) -> Self {
        Self { _task, res }
    }
}

impl Future for JoinFuture {
    type Output = Option<i32>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        #[cfg(feature = "thread")]
        return Poll::Ready(this.res.take());
        #[cfg(not(feature = "thread"))]
        {
            if this.res.is_none() {
                if this._task.state() == taskctx::TaskState::Exited {
                    Poll::Ready(Some(this._task.get_exit_code()))
                } else {
                    this._task.join(_cx.waker().clone());
                    Poll::Pending
                }
            } else {
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