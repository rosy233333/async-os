use std::thread;

fn main() {
    println!("std thread test");

    let ta = thread::spawn(|| {
        for _ in 0 .. 5 {
            println!("thread 1");
            thread::yield_now();
        }
    });

    let tb = thread::spawn(|| {
        for _ in 0 .. 5 {
            println!("thread 2");
            thread::yield_now();
        }
    });

    tb.join();
    ta.join();
}
