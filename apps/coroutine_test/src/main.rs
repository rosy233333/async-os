#![no_std]
#![no_main]

extern crate async_std;
use alloc::vec;
use async_std::sync::Mutex;
static A: Mutex<i32> = Mutex::new(23);

use core::time::Duration;

#[async_std::async_main]
async fn main() -> isize {
    let mut b = A.lock().await;
    async_std::println!("Mutex locked: {:?}", *b);
    *b = 34;
    // drop(b);
    let j = async_std::task::spawn(async {
        let a = A.lock().await;
        async_std::println!("spawn Mutex locked: {:?}", *a);
        32
    })
    .join();
    async_std::task::sleep(Duration::from_secs(1)).await;
    drop(b);
    let res = j.await.unwrap();
    async_std::println!("res {}", res);
    async_std::task::sleep(Duration::from_secs(1)).await;
    for i in 0..100 {
        async_std::println!("for test preempt {}", i);
    }
    let mut tasks = vec![];
    for i in 0..100 {
        tasks.push(async_std::task::spawn(async move {
            async_std::println!("spawn new task: {:?}", i);
        }));
    }
    for task in tasks {
        let _ = task.join().await;
    }
    async_std::println!("all tasks done");
    0
}
