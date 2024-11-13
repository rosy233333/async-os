#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use std::prelude::*;

#[macro_use]
extern crate async_std as std;

macro_rules! path_to_str {
    ($path:expr) => {{
        $path.as_str() // String -> &str
    }};
}

mod cmd;

const LF: u8 = b'\n';
const CR: u8 = b'\r';
const DL: u8 = b'\x7f';
const BS: u8 = b'\x08';
const SPACE: u8 = b' ';

const MAX_CMD_LEN: usize = 256;

async fn print_prompt() {
    print!(
        "arceos:{}$ ",
        path_to_str!(std::env::current_dir().await.unwrap())
    );
    std::io::stdout().flush().await.unwrap();
}

#[async_main]
async fn main() -> isize {
    let mut stdin = std::io::stdin();
    let mut stdout = std::io::stdout();

    let mut buf = [0; MAX_CMD_LEN];
    let mut cursor = 0;
    cmd::run_cmd("help".as_bytes()).await;
    print_prompt().await;

    loop {
        if stdin.read(&mut buf[cursor..cursor + 1]).await.ok() != Some(1) {
            continue;
        }
        if buf[cursor] == b'\x1b' {
            buf[cursor] = b'^';
        }
        match buf[cursor] {
            CR | LF => {
                println!();
                if cursor > 0 {
                    cmd::run_cmd(&buf[..cursor]).await;
                    cursor = 0;
                }
                print_prompt().await;
            }
            BS | DL => {
                if cursor > 0 {
                    stdout.write_all(&[BS, SPACE, BS]).await.unwrap();
                    cursor -= 1;
                }
            }
            0..=31 => {}
            c => {
                if cursor < MAX_CMD_LEN - 1 {
                    stdout.write_all(&[c]).await.unwrap();
                    cursor += 1;
                }
            }
        }
    }
}
