use core::future::Future;
use core::mem;
use core::pin::Pin;
use core::str;

use alloc::{string::String, vec::Vec};

use super::read_until_internal;
use crate::{self as io, ax_err, AsyncBufRead};
use core::task::{Context, Poll};

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct ReadLineFuture<'a, T: Unpin + ?Sized> {
    pub(crate) reader: &'a mut T,
    pub(crate) buf: &'a mut String,
    pub(crate) bytes: Vec<u8>,
    pub(crate) read: usize,
}

impl<T: AsyncBufRead + Unpin + ?Sized> Future for ReadLineFuture<'_, T> {
    type Output = io::Result<usize>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Self {
            reader,
            buf,
            bytes,
            read,
        } = &mut *self;
        let reader = Pin::new(reader);

        let ret = core::task::ready!(read_until_internal(reader, cx, b'\n', bytes, read));
        if str::from_utf8(&bytes).is_err() {
            Poll::Ready(
                ret.and_then(|_| ax_err!(InvalidData, "stream did not contain valid UTF-8")),
            )
        } else {
            #[allow(clippy::debug_assert_with_mut_call)]
            {
                debug_assert!(buf.is_empty());
                debug_assert_eq!(*read, 0);
            }

            // Safety: `bytes` is a valid UTF-8 because `str::from_utf8` returned `Ok`.
            mem::swap(unsafe { buf.as_mut_vec() }, bytes);
            Poll::Ready(ret)
        }
    }
}
