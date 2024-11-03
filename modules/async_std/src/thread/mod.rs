//! Native threads.

#[cfg(feature = "multitask")]
mod multi;
#[cfg(feature = "multitask")]
pub use multi::*;

use aos_api::task as api;

/// Current thread gives up the CPU time voluntarily, and switches to another
/// ready thread.
///
/// For single-threaded configuration (`multitask` feature is disabled), we just
/// relax the CPU and wait for incoming interrupts.
pub use api::{yield_now, sleep_until, sleep};
