use async_fs::api::port::{
    ConsoleWinSize, FileIO, FileIOType, OpenFlags, FIOCLEX, TCGETS, TIOCGPGRP, TIOCGWINSZ,
    TIOCSPGRP,
};
use async_io::SeekFrom;
use axerrno::{AxError, AxResult};
use axhal::console::{getchar, putchar, write_bytes};
use axlog::warn;
use core::{
    cell::UnsafeCell,
    future::Future,
    pin::Pin,
    task::{ready, Context, Poll},
};
use sync::Mutex;
extern crate alloc;
use alloc::string::String;
/// stdin file for getting chars from console
pub struct Stdin {
    pub flags: Mutex<OpenFlags>,
    pub line: UnsafeCell<String>,
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

impl FileIO for Stdin {
    fn read(self: Pin<&Self>, _cx: &mut Context<'_>, buf: &mut [u8]) -> Poll<AxResult<usize>> {
        // busybox
        if buf.len() == 1 {
            match getchar() {
                Some(c) => {
                    unsafe {
                        buf.as_mut_ptr().write_volatile(c);
                    }
                    Poll::Ready(Ok(1))
                }
                None => {
                    Poll::Pending
                }
            }
        } else {
            // user appilcation
            let line = unsafe { &mut *self.line.get() };
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
                    return Poll::Pending;
                }
            }
            let len = line.len();
            buf[..len].copy_from_slice(line.as_bytes());
            line.clear();
            Poll::Ready(Ok(len))
        }
    }

    fn write(self: Pin<&Self>, _cx: &mut Context<'_>, _buf: &[u8]) -> Poll<AxResult<usize>> {
        panic!("Cannot write to stdin!");
    }

    fn flush(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<AxResult<()>> {
        panic!("Flushing stdin")
    }

    /// whether the file is readable
    fn readable(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<bool> {
        Poll::Ready(true)
    }

    /// whether the file is writable
    fn writable(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<bool> {
        Poll::Ready(false)
    }

    /// whether the file is executable
    fn executable(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<bool> {
        Poll::Ready(false)
    }

    fn get_type(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<FileIOType> {
        Poll::Ready(FileIOType::Stdin)
    }

    fn ready_to_read(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<bool> {
        Poll::Ready(true)
    }

    fn ready_to_write(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<bool> {
        Poll::Ready(false)
    }

    fn ioctl(
        self: Pin<&Self>,
        _cx: &mut Context<'_>,
        request: usize,
        data: usize,
    ) -> Poll<AxResult<isize>> {
        Poll::Ready(match request {
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
        })
    }

    fn set_status(self: Pin<&Self>, cx: &mut Context<'_>, flags: OpenFlags) -> Poll<bool> {
        Poll::Ready(if flags.contains(OpenFlags::CLOEXEC) {
            *ready!(Pin::new(&mut self.flags.lock()).poll(cx)) = flags;
            true
        } else {
            false
        })
    }

    fn get_status(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<OpenFlags> {
        Pin::new(&mut self.flags.lock()).poll(_cx).map(|flag| *flag)
    }

    fn set_close_on_exec(self: Pin<&Self>, _cx: &mut Context<'_>, is_set: bool) -> Poll<bool> {
        let mut flag = ready!(Pin::new(&mut self.flags.lock()).poll(_cx));
        if is_set {
            // 设置close_on_exec位置
            *flag |= OpenFlags::CLOEXEC;
        } else {
            *flag &= !OpenFlags::CLOEXEC;
        }
        Poll::Ready(true)
    }

    fn as_any(&self) ->  &dyn core::any::Any {
        self
    }
}

impl FileIO for Stdout {
    fn read(self: Pin<&Self>, _cx: &mut Context<'_>, _buf: &mut [u8]) -> Poll<AxResult<usize>> {
        panic!("Cannot read from stdout!");
    }

    fn write(self: Pin<&Self>, _cx: &mut Context<'_>, _buf: &[u8]) -> Poll<AxResult<usize>> {
        write_bytes(_buf);
        Poll::Ready(Ok(_buf.len()))
    }

    fn flush(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<AxResult<()>> {
        // stdout is always flushed
        Poll::Ready(Ok(()))
    }

    fn seek(self: Pin<&Self>, _cx: &mut Context<'_>, _pos: SeekFrom) -> Poll<AxResult<u64>> {
        Poll::Ready(Err(AxError::Unsupported)) // 如果没有实现seek, 则返回Unsupported
    }

    fn executable(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<bool> {
        Poll::Ready(false)
    }

    fn readable(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<bool> {
        Poll::Ready(false)
    }

    fn writable(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<bool> {
        Poll::Ready(true)
    }

    fn get_type(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<FileIOType> {
        Poll::Ready(FileIOType::Stdout)
    }

    fn ready_to_read(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<bool> {
        Poll::Ready(false)
    }

    fn ready_to_write(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<bool> {
        Poll::Ready(true)
    }

    fn set_status(self: Pin<&Self>, cx: &mut Context<'_>, flags: OpenFlags) -> Poll<bool> {
        Poll::Ready(if flags.contains(OpenFlags::CLOEXEC) {
            *ready!(Pin::new(&mut self.flags.lock()).poll(cx)) = flags;
            true
        } else {
            false
        })
    }

    fn get_status(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<OpenFlags> {
        Pin::new(&mut self.flags.lock())
            .poll(_cx)
            .map(|flags| *flags)
    }

    fn set_close_on_exec(self: Pin<&Self>, _cx: &mut Context<'_>, is_set: bool) -> Poll<bool> {
        let mut flag = ready!(Pin::new(&mut self.flags.lock()).poll(_cx));
        if is_set {
            // 设置close_on_exec位置
            *flag |= OpenFlags::CLOEXEC;
        } else {
            *flag &= !OpenFlags::CLOEXEC;
        }
        Poll::Ready(true)
    }

    fn ioctl(
        self: Pin<&Self>,
        _cx: &mut Context<'_>,
        request: usize,
        data: usize,
    ) -> Poll<AxResult<isize>> {
        Poll::Ready(match request {
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
        })
    }

    fn as_any(&self) ->  &dyn core::any::Any {
        self
    }
}

impl FileIO for Stderr {
    fn read(self: Pin<&Self>, _cx: &mut Context<'_>, _buf: &mut [u8]) -> Poll<AxResult<usize>> {
        panic!("Cannot read from stderr!");
    }

    fn write(self: Pin<&Self>, _cx: &mut Context<'_>, _buf: &[u8]) -> Poll<AxResult<usize>> {
        write_bytes(_buf);
        Poll::Ready(Ok(_buf.len()))
    }

    fn flush(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<AxResult<()>> {
        // stderr is always flushed
        Poll::Ready(Ok(()))
    }

    fn seek(self: Pin<&Self>, _cx: &mut Context<'_>, _pos: SeekFrom) -> Poll<AxResult<u64>> {
        Poll::Ready(Err(AxError::Unsupported)) // 如果没有实现seek, 则返回Unsupported
    }

    fn executable(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<bool> {
        Poll::Ready(false)
    }

    fn readable(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<bool> {
        Poll::Ready(false)
    }

    fn writable(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<bool> {
        Poll::Ready(true)
    }

    fn get_type(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<FileIOType> {
        Poll::Ready(FileIOType::Stderr)
    }

    fn ready_to_read(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<bool> {
        Poll::Ready(false)
    }

    fn ready_to_write(self: Pin<&Self>, _cx: &mut Context<'_>) -> Poll<bool> {
        Poll::Ready(true)
    }

    fn ioctl(
        self: Pin<&Self>,
        _cx: &mut Context<'_>,
        request: usize,
        data: usize,
    ) -> Poll<AxResult<isize>> {
        Poll::Ready(match request {
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
        })
    }

    fn as_any(&self) ->  &dyn core::any::Any {
        self
    }
}
