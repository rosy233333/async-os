#![feature(anonymous_pipe)]
#![feature(noop_waker)]
#![feature(future_join)]

use std::collections::VecDeque;
use std::future::Future;
use std::os::fd::AsRawFd;
use core::pin::Pin;
use std::pipe::{pipe, PipeReader, PipeWriter};
use std::task::{Context, Waker};
use syscalls::{sys_read, sys_write};

fn main() {
    let (pipe_reader, pipe_writer) = pipe().unwrap();
    let a = reader(pipe_reader);
    let b = writer(pipe_writer);
    let waker = Waker::noop();
    let mut cx = Context::from_waker(&waker);
    let mut fut_handler = VecDeque::<Pin<Box<dyn Future<Output = ()>>>>::new();
    fut_handler.push_back(Box::pin(a));
    fut_handler.push_back(Box::pin(b));
    while let Some(mut fut) = fut_handler.pop_front() {
        if fut.as_mut().poll(&mut cx).is_pending() {
            fut_handler.push_back(fut);
        }
    }
    
    println!("pipetest ok!");
}

async fn reader(pipe_reader: PipeReader) {
    let mut buf = [0; 1024];
    let n = sys_read(pipe_reader.as_raw_fd(), &mut buf).await.unwrap();
    println!("read {} bytes: {:?}", n, &buf[..n]);
}

async fn writer(pipe_writer: PipeWriter) {
    sys_write(pipe_writer.as_raw_fd(), b"Hello, world!").await.unwrap();
}
