#![forbid(unsafe_code)]
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

pub use error::{Error, KeychainFailure, Result};
pub use key::Key;
pub use keychain::{KeyStore, Keychain};
pub use model::{Entry, EntryId, Format, FormatKind, NewEntry, Pin, PinSlot, Timestamp};
pub use store::{Options, Store};
pub use vault::{
    KdfParams, MasterPassword, NewSecret, RecoveryKey, Secret, SecretId, SecretLabel, Vault,
    VaultOptions, VaultStatus,
};

pub const fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
