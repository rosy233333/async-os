#![no_std]
#![no_main]

use core::time::Duration;

use async_std::{println, sync::Mutex, task::{sleep, spawn, yield_now}};

extern crate async_std;

static A: Mutex<i32> = Mutex::new(23);


#[async_std::async_main]
async fn main() -> i32 {
    yield_now();
    println!("yield end");
    let a = A.lock();
    println!("a = {:?}", a);
    let task = spawn(async {
        let b = A.lock();
        println!("b = {:?}", b);
        78
    });
    sleep(Duration::from_secs(1));
    drop(a);
    let res = task.join().unwrap();
    println!("res = {:?}", res);
    0
}