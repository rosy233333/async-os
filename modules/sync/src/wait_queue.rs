use alloc::sync::Arc;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};
use spinlock::SpinNoIrq;
#[cfg(feature = "thread")]
use task_api::{block_current, current_task};
use task_api::{cancel_alarm, set_alarm_wakeup, WaitTaskList, WaitWakerNode};

#[cfg(feature = "irq")]
use axhal::time::{current_time, TimeValue};

pub struct WaitQueue {
    // Support queue lock by external caller,use SpinNoIrq
    // Arceos SpinNoirq current implementation implies irq_save,
    // so it can be nested
    // use linked list has good performance
    queue: SpinNoIrq<WaitTaskList>,
}

impl WaitQueue {
    /// Creates an empty wait queue.
    pub const fn new() -> Self {
        Self {
            queue: SpinNoIrq::new(WaitTaskList::new()),
        }
    }

    /// 当前任务进入阻塞状态，将 cx 注册到等待队列中
    pub fn wait<'a>(&'a self) -> WaitFuture<'a> {
        #[cfg(feature = "thread")]
        {
            let waker = current_task().waker();
            let waker_node = Arc::new(WaitWakerNode::new(waker));
            self.queue.lock().prepare_to_wait(waker_node.clone());
            block_current();
            self.queue.lock().remove(&waker_node);
        }
        WaitFuture {
            _wq: self,
            _flag: false,
        }
    }

    /// 当前任务等待某个条件成功
    pub fn wait_until<'a, F>(&'a self, _condition: F) -> WaitUntilFuture<'a, F>
    where
        F: Fn() -> bool + Unpin,
    {
        #[cfg(feature = "thread")]
        {
            let waker = current_task().waker();
            let waker_node = Arc::new(WaitWakerNode::new(waker));
            loop {
                if _condition() {
                    break;
                }
                self.queue.lock().prepare_to_wait(waker_node.clone());
                block_current();
            }
            self.queue.lock().remove(&waker_node);
        }
        WaitUntilFuture {
            _wq: self,
            _condition,
        }
    }

    /// 当前任务等待，直到 deadline
    /// 参数使用 deadline，如果使用 Duration，则会导致每次进入这个函数都会重新计算 deadline
    /// 从而导致一直无法唤醒
    #[cfg(feature = "irq")]
    pub fn wait_timeout<'a>(&'a self, _deadline: TimeValue) -> WaitTimeoutFuture<'a> {
        #[cfg(feature = "thread")]
        {
            let waker = current_task().waker();
            let waker_node = Arc::new(WaitWakerNode::new(waker.clone()));
            self.queue.lock().prepare_to_wait(waker_node.clone());
            set_alarm_wakeup(_deadline, waker.clone());
            block_current();

            cancel_alarm(&waker);
            self.queue.lock().remove(&waker_node);
            return WaitTimeoutFuture {
                res: Some(current_time() >= _deadline),
                _wq: self,
                _deadline,
                _flag: false,
            };
        }
        #[cfg(not(feature = "thread"))]
        WaitTimeoutFuture {
            res: None,
            _wq: self,
            _deadline,
            _flag: false,
        }
    }

    /// 当前任务等待条件满足或者到达deadline
    #[cfg(feature = "irq")]
    pub fn wait_timeout_until<'a, F>(
        &'a self,
        _deadline: TimeValue,
        _condition: F,
    ) -> WaitTimeoutUntilFuture<'a, F>
    where
        F: Fn() -> bool + Unpin,
    {
        #[cfg(feature = "thread")]
        {
            let waker = current_task().waker();
            let waker_node = Arc::new(WaitWakerNode::new(waker.clone()));
            let mut timeout = false;
            loop {
                if _condition() {
                    break;
                }
                self.queue.lock().prepare_to_wait(waker_node.clone());
                set_alarm_wakeup(_deadline, waker.clone());
                block_current();

                cancel_alarm(&waker);
                if current_time() >= _deadline {
                    timeout = true;
                    break;
                }
            }

            self.queue.lock().remove(&waker_node);
            return WaitTimeoutUntilFuture {
                _wq: self,
                _deadline,
                _condition,
                res: Some(timeout),
            };
        }
        #[cfg(not(feature = "thread"))]
        WaitTimeoutUntilFuture {
            _wq: self,
            _deadline,
            _condition,
            res: None,
        }
    }

    /// Wake up the given task in the wait queue.
    pub fn notify_task(&self, waker: &Waker) -> bool {
        self.queue.lock().notify_task(waker)
    }

    /// Wakes up one task in the wait queue, usually the first one.
    pub fn notify_one(&self) -> bool {
        self.queue.lock().notify_one()
    }

    /// Wakes all tasks in the wait queue.
    pub fn notify_all(&self) {
        self.queue.lock().notify_all()
    }
}

pub struct WaitFuture<'a> {
    _wq: &'a WaitQueue,
    _flag: bool,
}

impl<'a> Future for WaitFuture<'a> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        cfg_if::cfg_if! {
            if #[cfg(feature = "thread")] {
                return Poll::Ready(());
            } else if #[cfg(not(feature = "thread"))] {
                let waker_node = Arc::new(WaitWakerNode::new(_cx.waker().clone()));
                let Self { _wq, _flag } = self.get_mut();
                if !*_flag {
                    _wq.queue.lock().prepare_to_wait(waker_node);
                    Poll::Pending
                } else {
                    _wq.queue.lock().remove(&waker_node);
                    Poll::Ready(())
                }
            }
        }
    }
}

pub struct WaitUntilFuture<'a, F> {
    _wq: &'a WaitQueue,
    _condition: F,
}

impl<'a, F: Fn() -> bool + Unpin> Future for WaitUntilFuture<'a, F> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        cfg_if::cfg_if! {
            if #[cfg(feature = "thread")] {
                return Poll::Ready(());
            } else if #[cfg(not(feature = "thread"))] {
                let Self { _wq, _condition } = self.get_mut();
                let waker_node = Arc::new(WaitWakerNode::new(_cx.waker().clone()));
                if _condition() {
                    _wq.queue.lock().remove(&waker_node);
                    Poll::Ready(())
                } else {
                    _wq.queue.lock().prepare_to_wait(waker_node);
                    Poll::Pending
                }
            }
        }
    }
}

#[cfg(feature = "irq")]
pub struct WaitTimeoutFuture<'a> {
    res: Option<bool>,
    _wq: &'a WaitQueue,
    _deadline: TimeValue,
    _flag: bool,
}

#[cfg(feature = "irq")]
impl<'a> Future for WaitTimeoutFuture<'a> {
    type Output = bool;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self {
            res,
            _wq,
            _deadline,
            _flag,
        } = self.get_mut();
        cfg_if::cfg_if! {
            if #[cfg(feature = "thread")] {
                assert!(res.is_some());
                return Poll::Ready(res.unwrap());
            } else if #[cfg(not(feature = "thread"))] {
                if res.is_some() {
                    Poll::Ready(res.unwrap())
                } else {
                    let waker_node = Arc::new(WaitWakerNode::new(_cx.waker().clone()));
                    if !*_flag {
                        _wq.queue.lock().prepare_to_wait(waker_node);
                        set_alarm_wakeup(*_deadline, _cx.waker().clone());
                        Poll::Pending
                    } else {
                        cancel_alarm(_cx.waker());
                        _wq.queue.lock().remove(&waker_node);
                        Poll::Ready(current_time() >= *_deadline)
                    }
                }
            }
        }
    }
}

#[cfg(feature = "irq")]
pub struct WaitTimeoutUntilFuture<'a, F> {
    res: Option<bool>,
    _wq: &'a WaitQueue,
    _deadline: TimeValue,
    _condition: F,
}

#[cfg(feature = "irq")]
impl<'a, F: Fn() -> bool + Unpin> Future for WaitTimeoutUntilFuture<'a, F> {
    type Output = bool;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self {
            _wq,
            _deadline,
            _condition,
            res,
        } = self.get_mut();
        cfg_if::cfg_if! {
            if #[cfg(feature = "thread")] {
                assert!(res.is_some());
                return Poll::Ready(res.unwrap());
            } else if #[cfg(not(feature = "thread"))] {
                if res.is_some() {
                    Poll::Ready(res.unwrap())
                } else {
                    let waker_node = Arc::new(WaitWakerNode::new(_cx.waker().clone()));
                    let current_time = current_time();
                    if _condition() {
                        _wq.queue.lock().remove(&waker_node);
                        Poll::Ready(current_time >= *_deadline)
                    } else {
                        if current_time >= *_deadline {
                            cancel_alarm(_cx.waker());
                            _wq.queue.lock().remove(&waker_node);
                            Poll::Ready(true)
                        } else {
                            _wq.queue.lock().prepare_to_wait(waker_node);
                            set_alarm_wakeup(*_deadline, _cx.waker().clone());
                            Poll::Pending
                        }
                    }
                }
            }
        }
    }
}
