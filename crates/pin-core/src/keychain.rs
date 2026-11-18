use std::sync::Arc;

use keyring_core::{CredentialStore, Entry};
use zeroize::{Zeroize, Zeroizing};

use crate::error::{Error, KeychainFailure, Result};
use crate::key::Key;

pub trait KeyStore {
    fn load(&self) -> Result<Option<Key>>;
    fn save(&self, key: &Key) -> Result<()>;
}

#[derive(Debug)]
pub struct Keychain(Entry);

impl Keychain {
    pub const SERVICE: &'static str = "pin";
    pub const DATABASE_ACCOUNT: &'static str = "database-key";

    pub fn new(account: &str) -> Result<Self> {
        let store = platform_store().map_err(Error::KeychainUnavailable)?;
        store
            .build(Self::SERVICE, account, None)
            .map(Self)
            .map_err(keychain_error)
    }
}

impl KeyStore for Keychain {
    fn load(&self) -> Result<Option<Key>> {
        match self.0.get_secret() {
            Ok(bytes) => Key::from_slice(&Zeroizing::new(bytes)).map(Some),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(e) => Err(keychain_error(e)),
        }
    }

    fn save(&self, key: &Key) -> Result<()> {
        self.0.set_secret(key.as_bytes()).map_err(keychain_error)
    }
}

#[cfg(target_os = "macos")]
fn platform_store() -> keyring_core::Result<Arc<CredentialStore>> {
    Ok(apple_native_keyring_store::keychain::Store::new()?)
}

#[cfg(target_os = "windows")]
fn platform_store() -> keyring_core::Result<Arc<CredentialStore>> {
    Ok(windows_native_keyring_store::Store::new()?)
}

#[cfg(any(
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn platform_store() -> keyring_core::Result<Arc<CredentialStore>> {
    Ok(dbus_secret_service_keyring_store::Store::new()?)
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "windows",
    target_os = "linux",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
)))]
compile_error!("pin has no OS keychain store for this target");

// Callers hold the database lock file, so the existence check and the save cannot race another
// pin process. Creating a key next to an existing database would make it unreadable forever.
pub(crate) fn load_or_create(store: &impl KeyStore, database_exists: bool) -> Result<Key> {
    if let Some(key) = store.load()? {
        return Ok(key);
    }
    if database_exists {
        return Err(Error::KeyMissing);
    }
    let key = Key::generate()?;
    store.save(&key)?;
    tracing::info!("created database key in the keychain");
    Ok(key)
}

// Some keyring errors carry the raw stored bytes; they are wiped and never reach Debug output.
fn keychain_error(e: keyring_core::Error) -> Error {
    use keyring_core::Error as E;
    let (kind, source) = match e {
        E::BadEncoding(mut bytes) | E::BadDataFormat(mut bytes, _) => {
            bytes.zeroize();
            (KeychainFailure::BadFormat, None)
        }
        e @ E::NoStorageAccess(_) => (KeychainFailure::AccessDenied, Some(e)),
        e @ E::NoEntry => (KeychainFailure::NotFound, Some(e)),
        e @ E::Ambiguous(_) => (KeychainFailure::Ambiguous, Some(e)),
        e => (KeychainFailure::Platform, Some(e)),
    };
    Error::Keychain { kind, source }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    #[derive(Default)]
    struct Memory(RefCell<Option<Vec<u8>>>);

    impl KeyStore for Memory {
        fn load(&self) -> Result<Option<Key>> {
            self.0.borrow().as_deref().map(Key::try_from).transpose()
        }

        fn save(&self, key: &Key) -> Result<()> {
            *self.0.borrow_mut() = Some(key.as_bytes().to_vec());
            Ok(())
        }
    }

    #[test]
    fn key_is_created_once_then_reused() {
        let store = Memory::default();
        let first = load_or_create(&store, false).unwrap();
        let second = load_or_create(&store, true).unwrap();
        assert_eq!(first.as_bytes(), second.as_bytes());
        assert_ne!(first.as_bytes(), &[0; Key::LEN]);
    }

    #[test]
    fn stored_key_of_wrong_length_is_rejected() {
        let store = Memory(RefCell::new(Some(vec![1; 5])));
        assert!(matches!(
            load_or_create(&store, false),
            Err(Error::InvalidKeyLength)
        ));
    }

    #[test]
    fn missing_key_next_to_existing_database_is_not_recreated() {
        let store = Memory::default();
        assert!(matches!(
            load_or_create(&store, true),
            Err(Error::KeyMissing)
        ));
        assert!(store.0.borrow().is_none());
    }

    #[test]
    fn keychain_errors_drop_stored_bytes() {
        let error = keychain_error(keyring_core::Error::BadEncoding(b"leaked-key".to_vec()));
        assert!(matches!(
            error,
            Error::Keychain {
                kind: KeychainFailure::BadFormat,
                source: None
            }
        ));
        assert!(!format!("{error:?}").contains("108"));
        assert!(matches!(
            keychain_error(keyring_core::Error::NoEntry),
            Error::Keychain {
                kind: KeychainFailure::NotFound,
                source: Some(_)
            }
        ));
    }
}
