mod stdio;
pub use self::stdio::{stdin, stdout, Stdin, Stdout};
pub use stdio::__print_impl;

pub type Result<T> = async_io::Result<T>;

pub fn ax_console_read_byte() -> Option<u8> {
    axhal::console::getchar().map(|c| if c == b'\r' { b'\n' } else { c })
}

pub fn ax_console_write_bytes(buf: &[u8]) -> Result<usize> {
    axhal::console::write_bytes(buf);
    Ok(buf.len())
}

pub fn ax_console_write_fmt(args: core::fmt::Arguments) -> core::fmt::Result {
    axlog::print_fmt(args)
}
