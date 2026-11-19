mod codec;
mod crypto;
mod model;
mod recovery;
mod secrets;
mod session;

use std::fmt;
use std::time::Duration;

use rusqlite::Connection;
use zeroize::Zeroizing;

use crate::error::{Error, Result};

pub use model::{NewSecret, Secret, SecretId, SecretLabel};
pub use recovery::RecoveryKey;
pub(crate) use session::Session;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VaultStatus {
    Missing,
    Locked,
    Unlocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfParams {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
}

impl Default for KdfParams {
    fn default() -> Self {
        Self {
            memory_kib: 64 * 1024,
            iterations: 3,
            parallelism: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VaultOptions {
    pub auto_lock: Option<Duration>,
    pub kdf: KdfParams,
}

impl Default for VaultOptions {
    fn default() -> Self {
        Self {
            auto_lock: Some(Duration::from_mins(5)),
            kdf: KdfParams::default(),
        }
    }
}

pub struct MasterPassword(Zeroizing<String>);

impl MasterPassword {
    pub fn new(password: String) -> Result<Self> {
        let password = Zeroizing::new(password);
        if password.is_empty() {
            return Err(Error::EmptyPassword);
        }
        Ok(Self(password))
    }

    fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl fmt::Debug for MasterPassword {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MasterPassword(..)")
    }
}

pub struct Vault<'a> {
    conn: &'a mut Connection,
    session: &'a mut Option<Session>,
    options: VaultOptions,
}

impl fmt::Debug for Vault<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Vault")
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}

impl<'a> Vault<'a> {
    pub(crate) fn new(
        conn: &'a mut Connection,
        session: &'a mut Option<Session>,
        options: VaultOptions,
    ) -> Self {
        Self {
            conn,
            session,
            options,
        }
    }
}
