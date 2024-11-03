use axhal::time::current_time;
use lazy_init::LazyInit;
use spinlock::SpinNoIrq;
use timer_list::{TimeValue, TimerEvent, TimerList};
use core::task::Waker;

// TODO: per-CPU
static TIMER_LIST: LazyInit<SpinNoIrq<TimerList<TaskWakeupEvent>>> = LazyInit::new();

struct TaskWakeupEvent(Waker);

impl TimerEvent for TaskWakeupEvent {
    fn callback(self, _now: TimeValue) {
        self.0.wake();
    }
}

pub fn set_alarm_wakeup(deadline: TimeValue, waker: Waker) {
    let mut timer_list = TIMER_LIST.lock();
    timer_list.set(deadline, TaskWakeupEvent(waker));
    drop(timer_list)
}

pub fn cancel_alarm(waker: &Waker) {
    TIMER_LIST.lock().cancel(|t| Waker::will_wake(&t.0, waker));
}

pub fn check_events() {
    loop {
        let now = current_time();
        let event = TIMER_LIST.lock().expire_one(now);
        if let Some((_deadline, event)) = event {
            event.callback(now);
        } else {
            break;
        }
    }
}

pub fn init() {
    TIMER_LIST.init_by(SpinNoIrq::new(TimerList::new()));
}
