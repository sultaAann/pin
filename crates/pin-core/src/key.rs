use std::fmt;
use std::fmt::Write as _;

use zeroize::{Zeroize, Zeroizing};

use crate::error::{Error, Result};
use crate::random;

pub struct Key(Box<Zeroizing<[u8; Key::LEN]>>);

impl Key {
    pub const LEN: usize = 32;

    /// Moves the key out of `bytes` and zeroes the caller's buffer.
    pub fn from_bytes(bytes: &mut [u8; Self::LEN]) -> Self {
        let mut boxed = Box::new(Zeroizing::new([0u8; Self::LEN]));
        boxed.copy_from_slice(bytes);
        bytes.zeroize();
        Self(boxed)
    }

    pub fn from_slice(bytes: &[u8]) -> Result<Self> {
        Self::try_from(bytes)
    }

    pub fn generate() -> Result<Self> {
        let mut bytes = [0u8; Self::LEN];
        random::fill(&mut bytes)?;
        Ok(Self::from_bytes(&mut bytes))
    }

    pub fn as_bytes(&self) -> &[u8; Self::LEN] {
        &self.0
    }

    pub(crate) fn sqlcipher_pragma(&self) -> Zeroizing<String> {
        let mut sql = Zeroizing::new(String::with_capacity(Self::LEN * 2 + 24));
        sql.push_str("PRAGMA key = \"x'");
        for byte in self.0.iter() {
            let _ = write!(sql, "{byte:02x}");
        }
        sql.push_str("'\";");
        sql
    }
}

impl TryFrom<&[u8]> for Key {
    type Error = Error;

    fn try_from(bytes: &[u8]) -> Result<Self> {
        let mut array: [u8; Self::LEN] = bytes.try_into().map_err(|_| Error::InvalidKeyLength)?;
        Ok(Self::from_bytes(&mut array))
    }
}

impl fmt::Debug for Key {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Key(..)")
    }
}
