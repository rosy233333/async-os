#![no_std]
#![no_main]

extern crate async_std;
use async_std::sync::Mutex;
static A: Mutex<i32> = Mutex::new(23);

use core::time::Duration;

#[async_std::async_main]
async fn main() -> i32 {
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
    0
}
