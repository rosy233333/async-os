use crate::{Scheduler, Task, TaskRef};
use std::cell::RefCell;
use std::mem::ManuallyDrop;
use std::ops::Deref;
use std::sync::{Arc, Mutex};
use std::task::Waker;
use std::thread_local;

#[cfg(not(feature = "shared_scheduler"))]
thread_local! {
    pub static SCHEDULER: RefCell<Arc<Mutex<Scheduler>>> = RefCell::new(Arc::new(Mutex::new(Scheduler::new())));
}