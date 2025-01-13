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
    let listener = TcpListener::bind((LOCAL_IP, LOCAL_PORT)).await?;
    println!("listen on: {}", listener.local_addr().unwrap());

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
