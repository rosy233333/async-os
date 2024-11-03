//! Inspection and manipulation of the process’s environment.

#[cfg(feature = "fs")]
extern crate alloc;

#[cfg(feature = "fs")]
use {crate::io, alloc::string::String};

/// Returns the current working directory as a [`String`].
#[cfg(feature = "fs")]
pub async fn current_dir() -> io::Result<String> {
    aos_api::fs::ax_current_dir().await
}

/// Changes the current working directory to the specified path.
#[cfg(feature = "fs")]
pub async fn set_current_dir(path: &str) -> io::Result<()> {
    aos_api::fs::ax_set_current_dir(path).await
}
