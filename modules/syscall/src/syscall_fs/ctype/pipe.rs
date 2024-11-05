use async_fs::api::{FileIO, FileIOType, OpenFlags};
extern crate alloc;
use alloc::{
    boxed::Box,
    sync::{Arc, Weak},
};
use axerrno::AxResult;
use axlog::{info, trace};
use core::{
    future::Future,
    pin::Pin,
    task::{ready, Context, Poll},
};

use executor::yield_now;
use sync::Mutex;

/// IPC pipe
pub struct Pipe {
    #[allow(unused)]
    readable: bool,
    #[allow(unused)]
    writable: bool,
    buffer: Arc<Mutex<PipeRingBuffer>>,
    #[allow(unused)]
    flags: Mutex<OpenFlags>,
}

impl Pipe {
    /// create readable pipe
    pub fn read_end_with_buffer(buffer: Arc<Mutex<PipeRingBuffer>>, flags: OpenFlags) -> Self {
        Self {
            readable: true,
            writable: false,
            buffer,
            flags: Mutex::new(flags | OpenFlags::RDONLY),
        }
    }
    /// create writable pipe
    pub fn write_end_with_buffer(buffer: Arc<Mutex<PipeRingBuffer>>, flags: OpenFlags) -> Self {
        Self {
            readable: false,
            writable: true,
            buffer,
            flags: Mutex::new(flags | OpenFlags::WRONLY),
        }
    }
    /// is it set non block?
    pub async fn is_non_block(&self) -> bool {
        self.flags.lock().await.contains(OpenFlags::NON_BLOCK)
    }
}

const RING_BUFFER_SIZE: usize = 0x4000;

#[derive(Copy, Clone, PartialEq)]
enum RingBufferStatus {
    Full,
    Empty,
    Normal,
}

pub struct PipeRingBuffer {
    arr: [u8; RING_BUFFER_SIZE],
    head: usize,
    tail: usize,
    status: RingBufferStatus,
    write_end: Option<Weak<Pipe>>,
}

impl PipeRingBuffer {
    pub fn new() -> Self {
        Self {
            arr: [0; RING_BUFFER_SIZE],
            head: 0,
            tail: 0,
            status: RingBufferStatus::Empty,
            write_end: None,
        }
    }

    pub fn set_write_end(&mut self, write_end: &Arc<Pipe>) {
        self.write_end = Some(Arc::downgrade(write_end));
    }

    pub fn write_byte(&mut self, byte: u8) {
        self.status = RingBufferStatus::Normal;
        self.arr[self.tail] = byte;
        self.tail = (self.tail + 1) % RING_BUFFER_SIZE;
        if self.tail == self.head {
            self.status = RingBufferStatus::Full;
        }
    }
    pub fn read_byte(&mut self) -> u8 {
        self.status = RingBufferStatus::Normal;
        let c = self.arr[self.head];
        self.head = (self.head + 1) % RING_BUFFER_SIZE;
        if self.head == self.tail {
            self.status = RingBufferStatus::Empty;
        }
        c
    }
    pub fn available_read(&self) -> usize {
        if self.status == RingBufferStatus::Empty {
            0
        } else if self.tail > self.head {
            self.tail - self.head
        } else {
            self.tail + RING_BUFFER_SIZE - self.head
        }
    }
    pub fn available_write(&self) -> usize {
        if self.status == RingBufferStatus::Full {
            0
        } else {
            RING_BUFFER_SIZE - self.available_read()
        }
    }
    pub fn all_write_ends_closed(&self) -> bool {
        self.write_end.as_ref().unwrap().upgrade().is_none()
    }
}

/// Return (read_end, write_end)
pub async fn make_pipe(flags: OpenFlags) -> (Arc<Pipe>, Arc<Pipe>) {
    trace!("kernel: make_pipe");
    let buffer = Arc::new(Mutex::new(PipeRingBuffer::new()));
    let read_end = Arc::new(Pipe::read_end_with_buffer(buffer.clone(), flags));
    let write_end = Arc::new(Pipe::write_end_with_buffer(buffer.clone(), flags));
    buffer.lock().await.set_write_end(&write_end);
    (read_end, write_end)
}

impl FileIO for Pipe {
    fn read(self: Pin<&Self>, cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<AxResult<usize>> {
        let fut = async {
            assert!(self.readable);
            let want_to_read = buf.len();
            let mut buf_iter = buf.iter_mut();
            let mut already_read = 0usize;
            loop {
                let mut ring_buffer = self.buffer.lock().await;
                let loop_read = ring_buffer.available_read();
                info!("kernel: Pipe::read: loop_read = {}", loop_read);
                if loop_read == 0 {
                    if executor::current_executor().have_signals().await.is_some() {
                        return Err(axerrno::AxError::Interrupted);
                    }
                    info!(
                        "kernel: Pipe::read: all_write_ends_closed = {}",
                        ring_buffer.all_write_ends_closed()
                    );
                    if Arc::strong_count(&self.buffer) < 2 || ring_buffer.all_write_ends_closed() {
                        return Ok(already_read);
                    }

                    if self.is_non_block().await {
                        yield_now().await;
                        return Err(axerrno::AxError::WouldBlock);
                    }
                    drop(ring_buffer);
                    yield_now().await;
                    continue;
                }
                for _ in 0..loop_read {
                    if let Some(byte_ref) = buf_iter.next() {
                        *byte_ref = ring_buffer.read_byte();
                        already_read += 1;
                        if already_read == want_to_read {
                            return Ok(want_to_read);
                        }
                    } else {
                        break;
                    }
                }

                return Ok(already_read);
            }
        };
        let res = Box::pin(fut).as_mut().poll(cx);
        res
    }

    fn write(self: Pin<&Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<AxResult<usize>> {
        let fut = async {
            info!("kernel: Pipe::write");
            assert!(self.writable);
            let want_to_write = buf.len();
            let mut buf_iter = buf.iter();
            let mut already_write = 0usize;
            loop {
                let mut ring_buffer = self.buffer.lock().await;
                let loop_write = ring_buffer.available_write();
                if loop_write == 0 {
                    drop(ring_buffer);

                    if Arc::strong_count(&self.buffer) < 2 || self.is_non_block().await {
                        // 读入端关闭
                        return Ok(already_write);
                    }
                    yield_now().await;
                    continue;
                }

                // write at most loop_write bytes
                for _ in 0..loop_write {
                    if let Some(byte_ref) = buf_iter.next() {
                        ring_buffer.write_byte(*byte_ref);
                        already_write += 1;
                        if already_write == want_to_write {
                            drop(ring_buffer);
                            return Ok(want_to_write);
                        }
                    } else {
                        break;
                    }
                }
                return Ok(already_write);
            }
        };
        let res = Box::pin(fut).as_mut().poll(cx);
        res
    }

    fn executable(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<bool> {
        Poll::Ready(false)
    }

    fn readable(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<bool> {
        Poll::Ready(self.readable)
    }

    fn writable(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<bool> {
        Poll::Ready(self.writable)
    }

    fn get_type(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<FileIOType> {
        Poll::Ready(FileIOType::Pipe)
    }

    fn is_hang_up(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<bool> {
        if self.readable {
            let ring_buffer = ready!(Pin::new(&mut self.buffer.lock()).poll(cx));
            if ring_buffer.available_read() == 0 && ring_buffer.all_write_ends_closed() {
                // 写入端关闭且缓冲区读完了
                Poll::Ready(true)
            } else {
                Poll::Ready(false)
            }
        } else {
            Poll::Ready(Arc::strong_count(&self.buffer) < 2)
        }
    }

    fn ready_to_read(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<bool> {
        if !self.readable {
            Poll::Ready(false)
        } else {
            let ring_buffer = ready!(Pin::new(&mut self.buffer.lock()).poll(_cx));
            Poll::Ready(ring_buffer.available_read() != 0)
        }
    }

    fn ready_to_write(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<bool> {
        if !self.writable {
            Poll::Ready(false)
        } else {
            let ring_buffer = ready!(Pin::new(&mut self.buffer.lock()).poll(cx));
            Poll::Ready(ring_buffer.available_write() != 0)
        }
    }

    /// 获取文件状态
    fn get_status(self: Pin<&Self>, cx: &mut Context<'_>) -> Poll<OpenFlags> {
        Pin::new(&mut self.flags.lock())
            .poll(cx)
            .map(|flags| *flags)
    }

    /// 设置文件状态
    fn set_status(self: Pin<&Self>, cx: &mut Context<'_>, flags: OpenFlags) -> Poll<bool> {
        *ready!(Pin::new(&mut self.flags.lock()).poll(cx)) = flags;
        Poll::Ready(true)
    }

    /// 设置 close_on_exec 位
    /// 设置成功返回false
    fn set_close_on_exec(self: Pin<&Self>, cx: &mut Context<'_>, is_set: bool) -> Poll<bool> {
        let mut flags = ready!(Pin::new(&mut self.flags.lock()).poll(cx));
        if is_set {
            // 设置close_on_exec位置
            *flags |= OpenFlags::CLOEXEC;
        } else {
            *flags &= !OpenFlags::CLOEXEC;
        }
        Poll::Ready(true)
    }
}
