#![no_std]
#![allow(unused)]

extern crate alloc;

use core::{future::{poll_fn, Future}, pin::Pin, task::{Context, Poll}};

use alloc::boxed::Box;

pub fn run<F>(main_task_fn: F)
where F: (FnOnce() -> i32) + Send + 'static {
    task_management::init_main_processor(0, 1);
    task_management::start_main_processor(main_task_fn);
}

pub fn spawn<F>(f: F)
where F: FnOnce() -> i32 + Send + 'static {
    task_management::spawn_to_local(f);
}

pub fn spawn_async<F>(f: F)
where F: Future<Output = i32> + Send + 'static {
    task_management::spawn_to_local_async(f);
}

pub fn yield_now() -> impl Future<Output = ()> {
    #[cfg(feature = "thread")]
    {
        task_management::yield_current_to_local();
        poll_fn(|_cx|{ Poll::Ready(()) })
    }
    #[cfg(not(feature = "thread"))]
    task_management::yield_current_to_local_async()
}