#![no_std]
#![no_main]

extern crate async_std;
use alloc::vec::Vec;
use async_std::{sync::Mutex, time::Instant};
static A: Mutex<i32> = Mutex::new(23);

use core::time::Duration;

#[async_std::async_main]
async fn main() -> isize {
    funtional_test().await;
    // perfomance_test().await;
    0
}

async fn funtional_test() {
    const TASK_NUM: usize = 100;
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
            drop(a);
            async_std::task::yield_now().await;
            async_std::println!("after yield");
            old_a
        });
        handles.push(j);
    }
    // async_std::task::sleep(Duration::from_secs(1)).await;
    // async_std::println!("after sleep");
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
}

async fn perfomance_test() {
    const TASK_NUM: usize = 100;
    const YIELD_PER_TASK: usize = 100;
    let mut handles = Vec::new();
    let now = Instant::now();
    for _ in 0..TASK_NUM {
        let j = async_std::task::spawn(async {
            for i in 0..YIELD_PER_TASK {
                async_std::task::yield_now().await;
            }
        });
        handles.push(j);
    }
    let create_elapsed = now.elapsed();
    for handle in handles {
        let res = handle.join().await.unwrap();
    }
    let finish_elapsed = now.elapsed();
    async_std::println!("Performance test result:");
    async_std::println!("create_elapsed: {:?}", create_elapsed);
    async_std::println!("finish_elapsed: {:?}", finish_elapsed);
}
