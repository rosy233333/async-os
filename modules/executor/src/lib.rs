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
mod stdio;
pub mod link;
mod loader;
pub mod signal;

pub mod flags;
pub use loader::load_app;

pub use api::*;
pub use current::CurrentExecutor;
pub use executor::Executor;
pub type ExecutorRef = alloc::sync::Arc<Executor>;
pub use taskctx::{BaseScheduler, TaskRef, TaskId};
pub use executor::*;
pub use fd_manager::*;
pub use stdio::{Stdin, Stdout, Stderr};
pub use signal::*;
pub mod futex;

