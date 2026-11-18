#![deny(clippy::undocumented_unsafe_blocks)]

mod capture;
mod error;
mod watcher;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as platform;

#[cfg(not(target_os = "macos"))]
mod unsupported;
#[cfg(not(target_os = "macos"))]
use unsupported as platform;

pub use capture::{Capture, ChangeCount, MAX_CAPTURE_BYTES, Privacy, Skip};
pub use error::{Error, Result};
pub use platform::Clipboard;
pub use watcher::{DEFAULT_INTERVAL, WatchHandle, Watcher, watch};
