#![no_std]
#![no_main]

extern crate async_std;

#[async_std::async_main]
async fn main() -> i32 {
    async_std::thread::yield_now();
    async_std::println!("yield end");
    0
}