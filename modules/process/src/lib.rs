#![no_std]
#![feature(type_alias_impl_trait)]

extern crate alloc;
#[macro_use]
extern crate axlog;

mod api;
mod current;
mod fd_manager;
pub mod link;
mod loader;
mod process;
pub mod signal;
mod stdio;

pub mod flags;
pub use loader::load_app;

pub use api::*;
pub use current::CurrentProcess;
pub use process::Process;
pub type ProcessRef = alloc::sync::Arc<Process>;
pub use fd_manager::*;
pub use process::*;
pub use signal::*;
pub use stdio::{Stderr, Stdin, Stdout};
pub use taskctx::{BaseScheduler, TaskId, TaskRef};
pub mod futex;
