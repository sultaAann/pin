//! Core of pin: clipboard history model, encrypted storage, search, pins and secrets.

mod content;
mod error;
mod key;
mod model;
mod schema;
mod store;

pub use error::{Error, Result};
pub use key::Key;
pub use model::{Entry, EntryId, Format, FormatKind, NewEntry, PinSlot, Timestamp};
pub use store::{Options, Store};

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
