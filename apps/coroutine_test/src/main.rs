#![no_std]
#![no_main]

extern crate async_std;
use alloc::vec::Vec;
use async_std::sync::Mutex;
static A: Mutex<i32> = Mutex::new(23);
const TASK_NUM: usize = 100;

use core::time::Duration;

#[async_std::async_main]
async fn main() -> isize {
    let mut b = A.lock().await;
    async_std::println!("Mutex locked: {:?}", *b);
    *b = 0;

    let mut handles = Vec::new();
    for _ in 0..TASK_NUM {
        let j = async_std::task::spawn(async {
            let mut a = A.lock().await;
            let old_a = *a;
            async_std::println!("spawn Mutex locked: {:?}", old_a);
            *a = old_a + 1;
            old_a
        });
        handles.push(j);
    }
    // async_std::task::sleep(Duration::from_secs(1)).await;
    drop(b);
    for handle in handles {
        let res = handle.join().await.unwrap();
        async_std::println!("res {}", res);
    }
    assert_eq!(*A.lock().await, TASK_NUM as i32);
    async_std::println!("test passed!");
    // async_std::task::sleep(Duration::from_secs(1)).await;
    // for i in 0..100 {
    //     async_std::println!("for test preempt {}", i);
    // }
    0
}
