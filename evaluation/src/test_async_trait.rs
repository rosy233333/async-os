use async_utils::async_trait as my_async_trait;
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

#[my_async_trait]
pub trait SelfRead {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, Error>>;
}

use async_trait::async_trait;

#[async_trait]
pub trait BoxRead {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error>;
}

use std::fs::File;
use std::io::{Error, Read};

pub struct TestFile {
    pub path: String,
}

impl SelfRead for TestFile {
    fn poll_read(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<usize, Error>> {
        let mut file = File::open(&self.path).unwrap();
        let res = file.read(buf);
        drop(file);
        Poll::Ready(res)
    }
}

#[async_trait]
impl BoxRead for TestFile {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        let mut file = File::open(&self.path).unwrap();
        let res = file.read(buf);
        drop(file);
        res
    }
}

#[test]
fn test_async_read() {
    use core::task::Waker;
    use std::io::Write;
    use std::time::Instant;
    const READ_TIMES: usize = 2000000;
    const BUF_SIZE: usize = 8;

    let mut file = TestFile {
        path: String::from("./foo.txt"),
    };
    let mut buf = [0u8; BUF_SIZE];

    let waker = Waker::noop();
    let cx = &mut Context::from_waker(&waker);
    let mut time_elapse = Vec::new();
    for _ in 0..READ_TIMES {
        let curr = Instant::now();
        let _a = Box::pin(AsyncSelfRead::read(&mut file, &mut buf))
            .as_mut()
            .poll(cx);
        let elapse = Instant::now().duration_since(curr);
        time_elapse.push(elapse.as_nanos());
    }

    let mut async_read_out = File::create("./async_read_out.txt").unwrap();
    let mut res = format!("{:?}", time_elapse);
    res.remove(0);
    res.pop();
    let res_buf = res.as_bytes();
    async_read_out.write_all(&res_buf).unwrap();

    let mut time_elapse = Vec::new();
    for _ in 0..READ_TIMES {
        let curr = Instant::now();
        let _a = Box::pin(BoxRead::read(&mut file, &mut buf))
            .as_mut()
            .poll(cx);
        let elapse = Instant::now().duration_since(curr);
        time_elapse.push(elapse.as_nanos());
    }

    let mut box_async_read_out = File::create("./box_async_read_out.txt").unwrap();
    let mut res = format!("{:?}", time_elapse);
    res.remove(0);
    res.pop();
    let res_buf = res.as_bytes();
    box_async_read_out.write_all(&res_buf).unwrap();
}
