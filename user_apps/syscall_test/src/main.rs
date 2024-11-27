#![feature(anonymous_pipe)]

use core::str;
use std::os::fd::AsRawFd;
use std::pipe::{pipe, PipeReader, PipeWriter};
use std::task::Context;

use syscalls::{sys_read, sys_write};
fn main() {
    uruntime::init();

    println!("pipe test:");
    let (pipe_reader, pipe_writer) = pipe().unwrap();
    // 非阻塞情况下，先调用read，再调用write
    uruntime::spawn_raw(|| reader(pipe_reader), "pipe reader".into());
    uruntime::spawn_raw(|| writer(pipe_writer), "pipe writer".into());
    let now = std::time::Instant::now();
    loop {
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
        if now.elapsed().as_millis() > 100 {
            break;
        }
    }
}

async fn reader(pipe_reader: PipeReader) -> isize {
    let mut buf = [0; 1024];
    let n = sys_read(pipe_reader.as_raw_fd(), &mut buf).await.unwrap();
    println!("read {} bytes: {:?}", n, str::from_utf8(&buf[..n]));
    0
}

async fn writer(pipe_writer: PipeWriter) -> isize {
    let res = sys_write(pipe_writer.as_raw_fd(), b"Hello, world!")
        .await
        .unwrap();
    println!("{:?}", res);
    0
}
