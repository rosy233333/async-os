use core::mem;
use core::pin::Pin;
use core::str;

use alloc::string::String;
use alloc::vec::Vec;
use pin_project_lite::pin_project;

use super::read_until_internal;
use crate::stream::AsyncStream;
use crate::{self as io, ax_err, AsyncBufRead};
use core::task::{Context, Poll};

pin_project! {
    /// A stream of lines in a byte stream.
    ///
    /// This stream is created by the [`lines`] method on types that implement [`BufRead`].
    ///
    /// This type is an async version of [`std::io::Lines`].
    ///
    /// [`lines`]: trait.BufRead.html#method.lines
    /// [`BufRead`]: trait.BufRead.html
    /// [`std::io::Lines`]: https://doc.rust-lang.org/std/io/struct.Lines.html
    #[derive(Debug)]
    pub struct Lines<R> {
        #[pin]
        pub(crate) reader: R,
        pub(crate) buf: String,
        pub(crate) bytes: Vec<u8>,
        pub(crate) read: usize,
    }
}

impl<R: AsyncBufRead> AsyncStream for Lines<R> {
    type Item = io::Result<String>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let this = self.project();
        let n = core::task::ready!(read_line_internal(
            this.reader,
            cx,
            this.buf,
            this.bytes,
            this.read
        ))?;
        if n == 0 && this.buf.is_empty() {
            return Poll::Ready(None);
        }
        if this.buf.ends_with('\n') {
            this.buf.pop();
            if this.buf.ends_with('\r') {
                this.buf.pop();
            }
        }
        Poll::Ready(Some(Ok(mem::replace(this.buf, String::new()))))
    }
}

pub fn read_line_internal<R: AsyncBufRead + ?Sized>(
    reader: Pin<&mut R>,
    cx: &mut Context<'_>,
    buf: &mut String,
    bytes: &mut Vec<u8>,
    read: &mut usize,
) -> Poll<io::Result<usize>> {
    let ret = core::task::ready!(read_until_internal(reader, cx, b'\n', bytes, read));
    if str::from_utf8(&bytes).is_err() {
        Poll::Ready(ret.and_then(|_| ax_err!(InvalidData, "stream did not contain valid UTF-8")))
    } else {
        debug_assert!(buf.is_empty());
        debug_assert_eq!(*read, 0);
        // Safety: `bytes` is a valid UTF-8 because `str::from_utf8` returned `Ok`.
        mem::swap(unsafe { buf.as_mut_vec() }, bytes);
        Poll::Ready(ret)
    }
}
