use std::task::Context;

fn main() {
    uruntime::init();

    println!("batch syscall test:");
    uruntime::spawn_raw(|| dispatcher(), "dispatcher".into());
    while let Some(task) = uruntime::pick_next_task() {
        println!("run: {}", task.id_name());
        uruntime::CurrentTask::init_current(task);
        let curr = uruntime::current_task();
        let waker = curr.waker();
        let mut cx = Context::from_waker(&waker);
        match curr.get_fut().as_mut().poll(&mut cx) {
            std::task::Poll::Ready(exit_code) => {
                println!("task is ready: {}", exit_code);
                uruntime::CurrentTask::clean_current();
            }
            std::task::Poll::Pending => {
                println!("task is pending");
                uruntime::CurrentTask::clean_current_without_drop();
            }
        }
    }
}

async fn dispatcher() -> isize {
    let cfg = uruntime::init_batch_async_syscall();
    println!("{:#X?}", cfg);
    uruntime::issue_syscall(&cfg);
    std::thread::sleep(std::time::Duration::from_secs(1));
    0
}
