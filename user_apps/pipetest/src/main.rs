#![feature(anonymous_pipe)]
#![feature(noop_waker)]
#![feature(future_join)]

mod implementation;
use implementation::pipe_test;

fn main() {
    user_lib::run(|| {
        pipe_test();
        0
    });
}
