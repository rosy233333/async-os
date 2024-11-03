#![no_std]
#![feature(doc_cfg)]
#![feature(async_iterator)]

extern crate alloc;
extern crate arch_boot;

pub mod io;
pub mod sync;
pub mod task;
pub mod env;
pub mod os;
pub mod time;
pub mod prelude;

#[cfg(feature = "fs")]
pub mod fs;
#[cfg(feature = "net")]
pub mod net;


#[macro_use]
mod macros;

pub use async_utils::async_main;