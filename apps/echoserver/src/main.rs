#![no_std]
#![no_main]

use alloc::vec::Vec;
use async_std::io;
use async_std::net::{TcpListener, TcpStream};
use async_std::prelude::{Read, Write};

#[macro_use]
extern crate async_std;

const LOCAL_IP: &str = "0.0.0.0";
const LOCAL_PORT: u16 = 5555;

#[async_std::async_main]
async fn main() -> isize {
    println!("Hello, echo server!");
    accept_loop().await.expect("test echo server failed");
    0
}

async fn accept_loop() -> io::Result<()> {
    let listener = TcpListener::bind((LOCAL_IP, 5555)).await?;
    let local_addr = listener.local_addr().unwrap();
    println!("listen on: {}", local_addr);
    // for i in 0..10 {
    //     async_std::task::spawn(async move {
    //         let buf = [0u8, 1u8, 2u8, 3u8, 4u8, 5u8, 6u8, 7u8];
    //         let mut stream = TcpStream::connect(local_addr).await.unwrap();
    //         println!("client {} connect ok!", i);
    //         stream.write_all(&buf).await;
    //         println!("client {} write ok!", i);
    //         let mut res: Vec<u8> = Vec::new();
    //         stream.read_to_end(&mut res).await;
    //         println!("client {} read ok!", i);
    //         assert_eq!(&res, reverse(&buf).as_slice());
    //         println!("client {} test ok!", i);
    //     });
    // }

    let mut i = 0;
    loop {
        match listener.accept().await {
            Ok((stream, addr)) => {
                println!("new client {}: {}", i, addr);
                async_std::task::spawn(async move {
                    match echo_server(stream).await {
                        Err(e) => println!("client connection error: {:?}", e),
                        Ok(()) => println!("client {} closed successfully", i),
                    }
                });
            }
            Err(e) => return Err(e),
        }
        i += 1;
    }
}

async fn echo_server(mut stream: TcpStream) -> io::Result<()> {
    let mut buf = [0u8; 1024];
    loop {
        let n = stream.read(&mut buf).await?;
        if n == 0 {
            return Ok(());
        }
        stream.write_all(reverse(&buf[..n]).as_slice()).await?;
    }
}

fn reverse(buf: &[u8]) -> Vec<u8> {
    let mut lines = buf
        .split(|&b| b == b'\n')
        .map(Vec::from)
        .collect::<Vec<_>>();
    for line in lines.iter_mut() {
        line.reverse();
    }
    lines.join(&b'\n')
}
