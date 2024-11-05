#![cfg_attr(not(test), no_std)]
#![cfg_attr(test, feature(noop_waker))]

#[cfg(test)]
mod test_async_trait;
