use async_fs::api::port::{
    async_trait, ConsoleWinSize, FileIO, FileIOType, OpenFlags, FIOCLEX, TCGETS, TIOCGPGRP,
    TIOCGWINSZ, TIOCSPGRP,
};
use async_io::SeekFrom;
use axerrno::{AxError, AxResult};
use axhal::console::{getchar, putchar, write_bytes};
use axlog::warn;
use core::task::Poll;
use sync::Mutex;

extern crate alloc;
use alloc::{boxed::Box, string::String};
/// stdin file for getting chars from console
pub struct Stdin {
    pub flags: Mutex<OpenFlags>,
}

unsafe impl Send for Stdin {}
unsafe impl Sync for Stdin {}

/// stdout file for putting chars to console
pub struct Stdout {
    pub flags: Mutex<OpenFlags>,
}

unsafe impl Send for Stdout {}
unsafe impl Sync for Stdout {}

/// stderr file for putting chars to console
pub struct Stderr {
    #[allow(unused)]
    pub flags: Mutex<OpenFlags>,
}

unsafe impl Send for Stderr {}
unsafe impl Sync for Stderr {}

pub const LF: u8 = 0x0au8;
pub const CR: u8 = 0x0du8;
pub const DL: u8 = 0x7fu8;
pub const BS: u8 = 0x08u8;

pub const SPACE: u8 = 0x20u8;

pub const BACKSPACE: [u8; 3] = [BS, SPACE, BS];

#[async_trait]
impl FileIO for Stdin {
    async fn read(&self, buf: &mut [u8]) -> AxResult<usize> {
        // busybox
        if buf.len() == 1 {
            core::future::poll_fn(|cx| match getchar() {
                Some(c) => {
                    unsafe {
                        buf.as_mut_ptr().write_volatile(c);
                    }
                    Poll::Ready(Ok(1))
                }
                None => {
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
            })
            .await
        } else {
            // user appilcation
            let mut line = String::new();
            loop {
                let c = getchar();
                if let Some(c) = c {
                    match c {
                        LF | CR => {
                            // convert '\r' to '\n'
                            line.push('\n');
                            putchar(b'\n');
                            break;
                        }
                        BS | DL => {
                            if !line.is_empty() {
                                write_bytes(&BACKSPACE);
                                line.pop();
                            }
                        }
                        _ => {
                            // echo
                            putchar(c);
                            line.push(c as char);
                        }
                    }
                } else {
                    let _ = core::future::poll_fn(|cx| {
                        cx.waker().wake_by_ref();
                        Poll::<AxResult<usize>>::Pending
                    })
                    .await;
                }
            }
            let len = line.len();
            buf[..len].copy_from_slice(line.as_bytes());
            Ok(len)
        }
    }

    async fn write(&self, _buf: &[u8]) -> AxResult<usize> {
        panic!("Cannot write to stdin!");
    }

    async fn flush(&self) -> AxResult<()> {
        panic!("Flushing stdin")
    }

    /// whether the file is readable
    async fn readable(&self) -> bool {
        true
    }

    /// whether the file is writable
    async fn writable(&self) -> bool {
        false
    }

    /// whether the file is executable
    async fn executable(&self) -> bool {
        false
    }

    async fn get_type(&self) -> FileIOType {
        FileIOType::Stdin
    }

    async fn ready_to_read(&self) -> bool {
        true
    }

    async fn ready_to_write(&self) -> bool {
        false
    }

    async fn ioctl(&self, request: usize, data: usize) -> AxResult<isize> {
        match request {
            TIOCGWINSZ => {
                let winsize = data as *mut ConsoleWinSize;
                unsafe {
                    *winsize = ConsoleWinSize::default();
                }
                Ok(0)
            }
            TCGETS | TIOCSPGRP => {
                warn!("stdin TCGETS | TIOCSPGRP, pretend to be tty.");
                // pretend to be tty
                Ok(0)
            }

            TIOCGPGRP => {
                warn!("stdin TIOCGPGRP, pretend to be have a tty process group.");
                unsafe {
                    *(data as *mut u32) = 0;
                }
                Ok(0)
            }
            FIOCLEX => Ok(0),
            _ => Err(AxError::Unsupported),
        }
    }

    async fn set_status(&self, flags: OpenFlags) -> bool {
        if flags.contains(OpenFlags::CLOEXEC) {
            *self.flags.lock().await = flags;
            true
        } else {
            false
        }
    }

    async fn get_status(&self) -> OpenFlags {
        *self.flags.lock().await
    }

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

#[async_trait]
impl FileIO for Stdout {
    async fn read(&self, _buf: &mut [u8]) -> AxResult<usize> {
        panic!("Cannot read from stdout!");
    }

    async fn write(&self, buf: &[u8]) -> AxResult<usize> {
        write_bytes(buf);
        Ok(buf.len())
    }

    async fn flush(&self) -> AxResult<()> {
        // stdout is always flushed
        Ok(())
    }

    async fn seek(&self, _pos: SeekFrom) -> AxResult<u64> {
        Err(AxError::Unsupported) // 如果没有实现seek, 则返回Unsupported
    }

    async fn executable(&self) -> bool {
        false
    }

    async fn readable(&self) -> bool {
        false
    }

    async fn writable(&self) -> bool {
        true
    }

    async fn get_type(&self) -> FileIOType {
        FileIOType::Stdout
    }

    async fn ready_to_read(&self) -> bool {
        false
    }

    async fn ready_to_write(&self) -> bool {
        true
    }

    async fn set_status(&self, flags: OpenFlags) -> bool {
        if flags.contains(OpenFlags::CLOEXEC) {
            *self.flags.lock().await = flags;
            true
        } else {
            false
        }
    }

    async fn get_status(&self) -> OpenFlags {
        *self.flags.lock().await
    }

    async fn set_close_on_exec(&self, is_set: bool) -> bool {
        if is_set {
            // 设置close_on_exec位置
            *self.flags.lock().await |= OpenFlags::CLOEXEC;
        } else {
            *self.flags.lock().await &= !OpenFlags::CLOEXEC;
        }
        true
    }

    async fn ioctl(&self, request: usize, data: usize) -> AxResult<isize> {
        match request {
            TIOCGWINSZ => {
                let winsize = data as *mut ConsoleWinSize;
                unsafe {
                    *winsize = ConsoleWinSize::default();
                }
                Ok(0)
            }
            TCGETS | TIOCSPGRP => {
                warn!("stdout TCGETS | TIOCSPGRP, pretend to be tty.");
                // pretend to be tty
                Ok(0)
            }

            TIOCGPGRP => {
                warn!("stdout TIOCGPGRP, pretend to be have a tty process group.");
                unsafe {
                    *(data as *mut u32) = 0;
                }
                Ok(0)
            }
            FIOCLEX => Ok(0),
            _ => Err(AxError::Unsupported),
        }
    }
}

#[async_trait]
impl FileIO for Stderr {
    async fn read(&self, _buf: &mut [u8]) -> AxResult<usize> {
        panic!("Cannot read from stderr!");
    }

    async fn write(&self, buf: &[u8]) -> AxResult<usize> {
        write_bytes(buf);
        Ok(buf.len())
    }

    async fn flush(&self) -> AxResult<()> {
        // stderr is always flushed
        Ok(())
    }

    async fn seek(&self, _pos: SeekFrom) -> AxResult<u64> {
        Err(AxError::Unsupported) // 如果没有实现seek, 则返回Unsupported
    }

    async fn executable(&self) -> bool {
        false
    }

    async fn readable(&self) -> bool {
        false
    }

    async fn writable(&self) -> bool {
        true
    }

    async fn get_type(&self) -> FileIOType {
        FileIOType::Stderr
    }

    async fn ready_to_read(&self) -> bool {
        false
    }

    async fn ready_to_write(&self) -> bool {
        true
    }

    async fn ioctl(&self, request: usize, data: usize) -> AxResult<isize> {
        match request {
            TIOCGWINSZ => {
                let winsize = data as *mut ConsoleWinSize;
                unsafe {
                    *winsize = ConsoleWinSize::default();
                }
                Ok(0)
            }
            TCGETS | TIOCSPGRP => {
                warn!("stderr TCGETS | TIOCSPGRP, pretend to be tty.");
                // pretend to be tty
                Ok(0)
            }

            TIOCGPGRP => {
                warn!("stderr TIOCGPGRP, pretend to be have a tty process group.");
                unsafe {
                    *(data as *mut u32) = 0;
                }
                Ok(0)
            }
            FIOCLEX => Ok(0),
            _ => Err(AxError::Unsupported),
        }
    }
}
