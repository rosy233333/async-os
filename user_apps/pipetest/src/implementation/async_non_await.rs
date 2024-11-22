/// use with `syscalls` crate's `thread` feature.

use core::str;
use std::os::fd::AsRawFd;
use std::pipe::{pipe, PipeReader, PipeWriter};
use user_lib::{sys_read, sys_write};

#[cfg(feature = "blocking")]
static IS_BLOCKING: &str = "blocking";
#[cfg(not(feature = "blocking"))]
static IS_BLOCKING: &str = "non-blocking";

pub fn pipe_test() {
    println!("pipe test: async, non-await, {}", IS_BLOCKING);
    let (pipe_reader, pipe_writer) = pipe().unwrap();
    let mut buf = [0; 1024];
    #[cfg(not(feature = "blocking"))]
    {
        // 非阻塞情况下，先调用read，再调用write
        user_lib::spawn_async(async move { reader(pipe_reader, &mut buf).await });
        user_lib::spawn_async(async move { writer(pipe_writer).await });
    }
    #[cfg(feature = "blocking")]
    {
        // 阻塞情况下，先调用write，再调用read
        user_lib::spawn_async(async move { writer(pipe_writer).await });
        user_lib::spawn_async(async move { reader(pipe_reader, &mut buf).await });
    }
}

async fn reader(pipe_reader: PipeReader, mut buf: &mut [u8]) -> i32 {
    let n = sys_read(pipe_reader.as_raw_fd(), &mut buf).unwrap();
    println!("read {} bytes: {:?}", n, str::from_utf8(&buf[..n]));
    0
}

async fn writer(pipe_writer: PipeWriter) -> i32 {
    let res = sys_write(pipe_writer.as_raw_fd(), b"Hello, world!").unwrap();
    #[cfg(not(feature = "blocking"))]
    std::thread::sleep(core::time::Duration::from_millis(20)); // 非阻塞情况下，需要让出给执行read的内核协程
    println!("{:?}", res);
    0
}