#![no_std]
#![feature(doc_cfg)]
#![feature(async_iterator)]
#![feature(async_closure)]

extern crate alloc;
extern crate arch_boot;

pub mod env;
pub mod io;
pub mod os;
pub mod prelude;
pub mod sync;
pub mod task;
pub mod time;

#[cfg(feature = "fs")]
pub mod fs;
#[cfg(feature = "net")]
pub mod net;

#[macro_use]
mod macros;

pub use async_utils::async_main;
