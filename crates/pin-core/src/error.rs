use std::fmt;

use crate::model::{EntryId, PinSlot};
use crate::vault::SecretId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum KeychainFailure {
    BadFormat,
    AccessDenied,
    NotFound,
    Ambiguous,
    Platform,
}

impl fmt::Display for KeychainFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::BadFormat => "stored item has an unexpected format",
            Self::AccessDenied => "access denied",
            Self::NotFound => "item not found",
            Self::Ambiguous => "more than one matching item",
            Self::Platform => "platform failure",
        })
    }
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("wrong database key or not a pin database")]
    WrongKey,
    #[error("database schema version {found} is newer than supported {supported}")]
    UnsupportedSchema { found: i64, supported: i64 },
    #[error("entry {0} not found")]
    NotFound(EntryId),
    #[error("pin slot {0} is out of range {min}..={max}", min = PinSlot::MIN, max = PinSlot::MAX)]
    InvalidSlot(u8),
    #[error("entry has no formats")]
    Empty,
    #[error("keychain is unavailable")]
    KeychainUnavailable(#[source] keyring_core::Error),
    #[error("keychain: {kind}")]
    Keychain {
        kind: KeychainFailure,
        #[source]
        source: Option<keyring_core::Error>,
    },
    #[error("database key is missing from the keychain")]
    KeyMissing,
    #[error("key has an unexpected length")]
    InvalidKeyLength,
    #[error("system random generator failed")]
    Random(#[source] getrandom::Error),
    #[error("vault is not set up")]
    VaultMissing,
    #[error("vault already exists")]
    VaultExists,
    #[error("vault is locked")]
    VaultLocked,
    #[error("vault was changed by another process")]
    VaultChanged,
    #[error("wrong master password")]
    WrongPassword,
    #[error("invalid recovery key")]
    InvalidRecoveryKey,
    #[error("master password must not be empty")]
    EmptyPassword,
    #[error("secret name must not be empty")]
    EmptyName,
    #[error("secret {0} not found")]
    SecretNotFound(SecretId),
    #[error("invalid key derivation parameters")]
    InvalidKdfParams,
    #[error("key derivation failed")]
    Kdf(#[source] argon2::Error),
    #[error("data is too large to encrypt")]
    TooLarge,
    #[error("cannot lock the database file")]
    Lock(#[source] std::io::Error),
    #[error("encrypted data is corrupt or was tampered with")]
    Corrupt,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("no home directory to keep pin data in")]
    NoDataDir,
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
