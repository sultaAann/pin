use crate::model::{EntryId, SecretId};

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    #[error("wrong database key or not a pin database")]
    WrongKey,
    #[error("database schema version {found} is newer than supported {supported}")]
    UnsupportedSchema { found: i64, supported: i64 },
    #[error("entry {0} not found")]
    NotFound(EntryId),
    #[error("pin slot {0} is out of range 1..=9")]
    InvalidSlot(u8),
    #[error("entry has no formats")]
    Empty,
    #[error("keychain is unavailable")]
    KeychainUnavailable(#[source] keyring_core::Error),
    #[error("keychain: {message}")]
    Keychain {
        message: &'static str,
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
    #[error("encrypted data is corrupt or was tampered with")]
    Corrupt,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
