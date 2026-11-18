#![expect(clippy::unwrap_used)]

use std::cell::RefCell;
use std::fmt::Write as _;
use std::path::Path;

use pin_core::{KdfParams, Key, KeyStore, Options, VaultOptions};

pub fn key() -> Key {
    Key::from_bytes(&mut [7; Key::LEN])
}

pub fn options() -> Options {
    Options {
        vault: VaultOptions {
            kdf: KdfParams {
                memory_kib: 64,
                iterations: 1,
                parallelism: 1,
            },
            ..VaultOptions::default()
        },
        ..Options::default()
    }
}

pub fn raw(dir: &Path) -> rusqlite::Connection {
    let conn = rusqlite::Connection::open(dir.join("pin.db")).unwrap();
    let hex = key().as_bytes().iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    });
    conn.execute_batch(&format!("PRAGMA key = \"x'{hex}'\";"))
        .unwrap();
    conn
}

#[derive(Default)]
pub struct MemoryKeys(RefCell<Option<Vec<u8>>>);

impl KeyStore for MemoryKeys {
    fn load(&self) -> pin_core::Result<Option<Key>> {
        self.0.borrow().as_deref().map(Key::try_from).transpose()
    }

    fn save(&self, key: &Key) -> pin_core::Result<()> {
        *self.0.borrow_mut() = Some(key.as_bytes().to_vec());
        Ok(())
    }
}
