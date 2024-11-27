#![feature(anonymous_pipe)]

use core::str;
use spin::Once;
use std::os::fd::AsRawFd;
use std::pipe::{pipe, PipeReader, PipeWriter};
use std::task::Context;
use syscalls::{sys_read, sys_write};
use uruntime::{block_current, yield_now, SyscallItemQueue};
fn main() {
    uruntime::init();
    println!("batch syscall test:");
    uruntime::spawn_raw(|| dispatcher(), "dispatcher".into());
    new_connection(0);
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
                uruntime::CurrentTask::clean_current_without_drop();
            }
        }
    }
}

async fn dispatcher() -> isize {
    let cfg = uruntime::init_batch_async_syscall();
    println!("{:#X?}", cfg);
    SYSCALL_SEND_QUEUE.call_once(|| cfg.recv_channel);
    let recv_queue = cfg.recv_queue();
    uruntime::start(&cfg);
    let now = std::time::Instant::now();
    loop {
        if let Some(syscall_item) = recv_queue.dequeue() {
            syscall_item.waker().wake();
        } else {
            yield_now().await;
        }
        if now.elapsed().as_millis() > 200 {
            break 0;
        }
    }
}

static SYSCALL_SEND_QUEUE: Once<usize> = Once::new();

fn new_connection(idx: usize) {
    let (pipe_reader, pipe_writer) = pipe().unwrap();
    uruntime::spawn_raw(|| writer(pipe_writer), format!("pipe writer {}", idx));
    uruntime::spawn_raw(|| reader(pipe_reader), format!("pipe reader {}", idx));
}
async fn reader(pipe_reader: PipeReader) -> isize {
    let mut buf = [0; 1024];
    let mut syscall_fut = sys_read(pipe_reader.as_raw_fd(), &mut buf);
    let syscall_item = (&mut syscall_fut).into();
    let send_queue = unsafe { &mut *(*SYSCALL_SEND_QUEUE.get().unwrap() as *mut SyscallItemQueue) };
    let _ = send_queue.enqueue(syscall_item);
    block_current().await;
    let n = syscall_fut.await.unwrap();
    println!("read {} bytes: {:?}", n, str::from_utf8(&buf[..n]));
    0
}

async fn writer(pipe_writer: PipeWriter) -> isize {
    let mut syscall_fut = sys_write(pipe_writer.as_raw_fd(), b"Hello, world!");
    let syscall_item = (&mut syscall_fut).into();
    let send_queue = unsafe { &mut *(*SYSCALL_SEND_QUEUE.get().unwrap() as *mut SyscallItemQueue) };
    let _ = send_queue.enqueue(syscall_item);
    block_current().await;
    let res = syscall_fut.await.unwrap();
    println!("writer res is {:?}", res);
    0
}
