use std::fmt;
use std::fmt::Write as _;

use zeroize::{Zeroize, Zeroizing};

pub struct Key([u8; Key::LEN]);

impl Key {
    pub const LEN: usize = 32;

    pub fn from_bytes(bytes: &mut [u8; Self::LEN]) -> Self {
        let key = Self(*bytes);
        bytes.zeroize();
        key
    }

    pub fn as_bytes(&self) -> &[u8; Self::LEN] {
        &self.0
    }

    pub(crate) fn sqlcipher_pragma(&self) -> Zeroizing<String> {
        let mut sql = Zeroizing::new(String::with_capacity(Self::LEN * 2 + 24));
        sql.push_str("PRAGMA key = \"x'");
        for byte in &self.0 {
            let _ = write!(sql, "{byte:02x}");
        }
        sql.push_str("'\";");
        sql
    }
}

impl Drop for Key {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Key(..)")
    }
}
