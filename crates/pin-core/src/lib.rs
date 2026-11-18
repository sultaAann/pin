//! Core of pin: clipboard history model, encrypted storage, search, pins and secrets.

mod content;
mod error;
mod key;
mod keychain;
mod model;
mod random;
mod schema;
mod store;
mod vault;

pub use error::{Error, Result};
pub use key::Key;
pub use keychain::{KeyStore, Keychain};
pub use model::{
    Entry, EntryId, Format, FormatKind, NewEntry, NewSecret, PinSlot, Secret, SecretId, Timestamp,
};
pub use store::{Options, Store};
pub use vault::{KdfParams, MasterPassword, RecoveryKey, Vault, VaultOptions, VaultStatus};

pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
