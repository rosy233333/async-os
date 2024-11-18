/// use with `syscalls` crate's `thread` feature.

use core::str;
use std::os::fd::AsRawFd;
use std::pipe::{pipe, PipeReader, PipeWriter};
use syscalls::{sys_read, sys_write, Errno};

#[cfg(feature = "blocking")]
static IS_BLOCKING: &str = "blocking";
#[cfg(not(feature = "blocking"))]
static IS_BLOCKING: &str = "non-blocking";

pub fn pipe_test() {
    println!("pipe test: non-async, non-await, {}", IS_BLOCKING);

    let (pipe_reader, pipe_writer) = pipe().unwrap();
    let mut buf = [0; 1024];
    #[cfg(not(feature = "blocking"))]
    let _ = reader(&pipe_reader, &mut buf); // 在阻塞式系统调用的情景下，这句代码会阻塞整个pipe_test函数，导致无法进行后续写入操作，也无法唤醒自身。
    let _ = writer(&pipe_writer);
    std::thread::sleep(core::time::Duration::from_millis(20));
    let _ = reader(&pipe_reader, &mut buf);
    println!("pipetest ok!");
}

fn reader(pipe_reader: &PipeReader, mut buf: &mut [u8]) {
    match *sys_read(pipe_reader.as_raw_fd(), &mut buf) {
        Ok(n) => {
            println!("read {} bytes: {:?}", n, str::from_utf8(&buf[..n]));
        },
        #[cfg(not(feature = "blocking"))]
        Err(Errno::EAGAIN) => {
            println!("syscall receive EAGAIN");
        },
        _ => {
            panic!("unsupported error.");
        }
    }
}

fn writer(pipe_writer: &PipeWriter) {
    let res = sys_write(pipe_writer.as_raw_fd(), b"Hello, world!").unwrap();
    println!("{:?}", res);
}