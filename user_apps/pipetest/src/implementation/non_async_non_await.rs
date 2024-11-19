// 暂未完成：之后需要改为用户态线程库实现

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
    user_task_scheduler::run(|| {
        println!("pipe test: non-async, non-await, {}", IS_BLOCKING);
        let (pipe_reader, pipe_writer) = pipe().unwrap();
        let mut buf = [0; 1024];
        #[cfg(not(feature = "blocking"))]
        {
            user_task_scheduler::spawn(move || { reader(pipe_reader, &mut buf); 0 });
            user_task_scheduler::spawn(move || { writer(pipe_writer); 0 });
            loop {
                user_task_scheduler::yield_now();
            }
        }
        #[cfg(feature = "blocking")]
        {
            let tb = std::thread::spawn(move || { writer(pipe_writer); });
            let ta = std::thread::spawn(move || { reader(pipe_reader, &mut buf); });
            ta.join();
            tb.join();
        }
        println!("pipetest ok!");
        0
    });
}

fn reader(pipe_reader: PipeReader, mut buf: &mut [u8]) {
    let sysres = sys_read(pipe_reader.as_raw_fd(), &mut buf);
    loop {
        match *sysres {
            Ok(n) => {
                println!("read {} bytes: {:?}", n, str::from_utf8(&buf[..n]));
                return;
            },
            #[cfg(not(feature = "blocking"))]
            Err(Errno::EAGAIN) => {
                // println!("syscall receive EAGAIN");
                user_task_scheduler::yield_now();
            },
            _ => {
                panic!("unsupported error.");
            }
        }
    }
}

fn writer(pipe_writer: PipeWriter) {
    let res = sys_write(pipe_writer.as_raw_fd(), b"Hello, world!").unwrap();
    std::thread::sleep(core::time::Duration::from_millis(20)); // 让出给执行read的内核协程
    println!("{:?}", res);
}