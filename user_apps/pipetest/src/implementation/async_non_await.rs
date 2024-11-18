/// use with `syscalls` crate's `thread` feature.

use core::str;
use std::future::{Future, poll_fn};
use std::os::fd::AsRawFd;
use std::pipe::{pipe, PipeReader, PipeWriter};
use std::task::{Context, Poll, Waker};
use syscalls::{sys_read, sys_write, Errno};

#[cfg(feature = "blocking")]
static IS_BLOCKING: &str = "blocking";
#[cfg(not(feature = "blocking"))]
static IS_BLOCKING: &str = "non-blocking";

pub fn pipe_test() {
    println!("pipe test: async, non-await, {}", IS_BLOCKING);

    let (pipe_reader, pipe_writer) = pipe().unwrap();
    let mut buf = [0; 1024];
    let a = reader(pipe_reader, &mut buf);
    let b = writer(pipe_writer);
    let waker = Waker::noop();
    let mut cx = Context::from_waker(&waker);
    let mut futa = std::mem::ManuallyDrop::new(Box::pin(a));
    let mut futb = std::mem::ManuallyDrop::new(Box::pin(b));
    #[cfg(not(feature = "blocking"))]
    let _ = futa.as_mut().poll(&mut cx); // 在阻塞式系统调用的情景下，这句代码会阻塞整个pipe_test函数，导致无法进行后续写入操作，也无法唤醒自身。
    let _ = futb.as_mut().poll(&mut cx);
    std::thread::sleep(core::time::Duration::from_millis(20));
    let _ = futa.as_mut().poll(&mut cx);

    println!("pipetest ok!");
}

async fn reader(pipe_reader: PipeReader, mut buf: &mut [u8]) {
    poll_fn(move |_cx| {
        match *sys_read(pipe_reader.as_raw_fd(), &mut buf) {
            Ok(n) => {
                println!("read {} bytes: {:?}", n, str::from_utf8(&buf[..n]));
                Poll::Ready(())
            },
            #[cfg(not(feature = "blocking"))]
            Err(Errno::EAGAIN) => {
                println!("syscall receive EAGAIN");
                Poll::Pending
            },
            _ => {
                panic!("unsupported error.");
            }
        }
    }).await
}

async fn writer(pipe_writer: PipeWriter) {
    let res = sys_write(pipe_writer.as_raw_fd(), b"Hello, world!").unwrap();
    println!("{:?}", res);
}