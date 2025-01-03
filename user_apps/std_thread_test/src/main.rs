fn main() {
    ktask_test();
    // user_lib::run(utask_test);
}

fn ktask_test() -> i32 {
    println!("kernel task scheduler test");

    std::thread::spawn(||{
        for _ in 0..5 {
            println!("coroutine 2");
            std::thread::yield_now();
        }
        0
    });

    std::thread::spawn(||{
        for _ in 0..5 {
            println!("coroutine 3");
            std::thread::yield_now();
        }
        0
    });

    loop{
        std::thread::yield_now();
    }

    0
}

fn utask_test() -> i32 {
    println!("user task scheduler test");

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
