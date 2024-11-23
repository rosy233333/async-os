fn main() {
    user_lib::run(amain);
}

fn amain() -> i32 {
    println!("user task scheduler test");

    // user_task_scheduler::spawn(|| {
    //     for _ in 0 .. 5 {
    //         println!("thread 1");
    //         user_task_scheduler::yield_now();
    //     }
    //     0
    // });

    user_lib::spawn_async(async {
        for _ in 0..5 {
            println!("coroutine 2");
            user_lib::yield_now().await;
        }
        0
    });

    user_lib::spawn_async(async {
        for _ in 0..5 {
            println!("coroutine 3");
            user_lib::yield_now().await;
        }
        0
    });

    0
}
