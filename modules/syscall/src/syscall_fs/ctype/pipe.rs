use async_fs::api::{FileIO, FileIOType, OpenFlags};
extern crate alloc;
use alloc::{
    boxed::Box,
    sync::{Arc, Weak},
};
use axerrno::AxResult;
use axlog::{info, trace};

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

#[async_trait::async_trait]
impl FileIO for Pipe {
    async fn read(&self, buf: &mut [u8]) -> AxResult<usize> {
        assert!(self.readable().await);
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
    }

    async fn write(&self, buf: &[u8]) -> AxResult<usize> {
        info!("kernel: Pipe::write");
        assert!(self.writable().await);
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
    }

    async fn executable(&self) -> bool {
        false
    }
    async fn readable(&self) -> bool {
        self.readable
    }
    async fn writable(&self) -> bool {
        self.writable
    }

    async fn get_type(&self) -> FileIOType {
        FileIOType::Pipe
    }

    async fn is_hang_up(&self) -> bool {
        if self.readable {
            if self.buffer.lock().await.available_read() == 0
                && self.buffer.lock().await.all_write_ends_closed()
            {
                // 写入端关闭且缓冲区读完了
                true
            } else {
                false
            }
        } else {
            // 否则在写入端，只关心读入端是否被关闭
            Arc::strong_count(&self.buffer) < 2
        }
    }

    async fn ready_to_read(&self) -> bool {
        self.readable && self.buffer.lock().await.available_read() != 0
    }

    async fn ready_to_write(&self) -> bool {
        self.writable && self.buffer.lock().await.available_write() != 0
    }

    /// 设置文件状态
    async fn set_status(&self, flags: OpenFlags) -> bool {
        *self.flags.lock().await = flags;
        true
    }

    /// 获取文件状态
    async fn get_status(&self) -> OpenFlags {
        *self.flags.lock().await
    }

    /// 设置 close_on_exec 位
    /// 设置成功返回false
    async fn set_close_on_exec(&self, is_set: bool) -> bool {
        if is_set {
            // 设置close_on_exec位置
            *self.flags.lock().await |= OpenFlags::CLOEXEC;
        } else {
            *self.flags.lock().await &= !OpenFlags::CLOEXEC;
        }
        true
    }
}
