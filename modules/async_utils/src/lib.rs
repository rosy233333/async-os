//! 这个库用于提供同步与异步之间的兼容层函数，
//! 参考了 core::future::poll_fn 的实现

#![cfg_attr(not(test), no_std)]
#![allow(async_fn_in_trait)]
#![cfg_attr(test, feature(noop_waker))]

pub use afit::async_trait;
pub use async_main::async_main;
