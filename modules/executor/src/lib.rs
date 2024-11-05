#![no_std]
#![feature(asm_const)]
#![feature(type_alias_impl_trait)]

extern crate alloc;
#[macro_use]
extern crate axlog;

mod api;
mod current;
mod executor;
mod fd_manager;
pub mod link;
mod loader;
pub mod signal;
mod stdio;

pub mod flags;
pub use loader::load_app;

pub use api::*;
pub use current::CurrentExecutor;
pub use executor::Executor;
pub type ExecutorRef = alloc::sync::Arc<Executor>;
pub use executor::*;
pub use fd_manager::*;
pub use signal::*;
pub use stdio::{Stderr, Stdin, Stdout};
pub use taskctx::{BaseScheduler, TaskId, TaskRef};
pub mod futex;
