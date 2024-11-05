use crate::{stream::AsyncStream, AsyncRead, Error, Result};
use core::pin::Pin;
use core::task::{Context, Poll};

/// A stream over `u8` values of a reader.
///
/// This struct is generally created by calling [`bytes`] on a reader.
/// Please see the documentation of [`bytes`] for more details.
///
/// [`bytes`]: trait.Read.html#method.bytes
#[derive(Debug)]
pub struct Bytes<T> {
    pub(crate) inner: T,
}

impl<T: AsyncRead + Unpin> AsyncStream for Bytes<T> {
    type Item = Result<u8>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut byte = 0;

        let rd = Pin::new(&mut self.inner);

        match core::task::ready!(rd.read(cx, core::slice::from_mut(&mut byte))) {
            Ok(0) => Poll::Ready(None),
            Ok(..) => Poll::Ready(Some(Ok(byte))),
            Err(ref e) if *e == Error::Interrupted => Poll::Pending,
            Err(e) => Poll::Ready(Some(Err(e))),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::read::Read;
    use crate::AsyncStream;
    use crate::Cursor;
    use core::task::{Context, Poll, Waker};

    #[test]
    fn test_bytes_basics() -> crate::Result<()> {
        let raw: Vec<u8> = vec![0, 1, 2, 3, 4, 5, 6, 7, 8];
        let source = Cursor::new(raw.clone());

        let mut s = source.bytes();
        let mut result = Vec::new();

        let waker = Waker::noop();
        let mut cx = Context::from_waker(waker);
        while let Poll::Ready(Some(byte)) =
            AsyncStream::poll_next(core::pin::Pin::new(&mut s), &mut cx)
        {
            let byte = byte?;
            std::println!("byte: {}", byte);
            result.push(byte);
        }

        assert_eq!(result, raw);

        Ok(())
    }
}
