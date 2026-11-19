#![forbid(unsafe_code)]
//! Core of pin: clipboard history model, encrypted storage, search, pins and secrets.

mod content;
mod error;
mod filter;
mod key;
mod keychain;
mod model;
mod paths;
mod random;
mod schema;
mod store;
mod thumbnail;
mod vault;

pub use error::{Error, KeychainFailure, Result};
pub use filter::Filter;
pub use key::Key;
pub use keychain::{KeyStore, Keychain};
pub use model::{Entry, EntryId, Format, FormatKind, NewEntry, Pin, PinSlot, Timestamp};
pub use paths::default_database_path;
pub use store::{Details, Options, Store, match_ranges};
pub use thumbnail::Thumbnail;
pub use vault::{
    KdfParams, MasterPassword, NewSecret, RecoveryKey, Secret, SecretId, SecretLabel, Vault,
    VaultOptions, VaultStatus,
};

pub const fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
