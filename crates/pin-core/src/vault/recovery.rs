use std::fmt;
use std::str::FromStr;

use zeroize::Zeroizing;

use super::crypto::{KEY_LEN, SecretKey};
use crate::error::{Error, Result};
use crate::random;

const ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const SYMBOLS: usize = (KEY_LEN * 8).div_ceil(5);
const GROUP: usize = 4;

#[must_use = "the recovery key is shown once and cannot be retrieved later"]
pub struct RecoveryKey(Zeroizing<[u8; KEY_LEN]>);

impl RecoveryKey {
    pub(super) fn generate() -> Result<Self> {
        let mut key = Zeroizing::new([0u8; KEY_LEN]);
        random::fill(key.as_mut_slice())?;
        Ok(Self(key))
    }

    pub fn parse(text: &str) -> Result<Self> {
        text.parse()
    }

    pub fn to_display(&self) -> Zeroizing<String> {
        let mut text = Zeroizing::new(String::with_capacity(SYMBOLS + SYMBOLS / GROUP));
        let mut push = |value: u16, count: &mut usize| {
            if *count > 0 && (*count).is_multiple_of(GROUP) {
                text.push('-');
            }
            text.push(char::from(ALPHABET[usize::from(value & 31)]));
            *count += 1;
        };
        let mut count = 0;
        let mut acc: u16 = 0;
        let mut bits = 0;
        for &byte in self.0.iter() {
            acc = (acc << 8) | u16::from(byte);
            bits += 8;
            while bits >= 5 {
                bits -= 5;
                push(acc >> bits, &mut count);
                acc &= (1 << bits) - 1;
            }
        }
        if bits > 0 {
            push(acc << (5 - bits), &mut count);
        }
        text
    }

    pub(super) fn wrapping_key(&self) -> SecretKey {
        SecretKey::derived("pin vault recovery key v1", self.0.as_slice())
    }
}

impl FromStr for RecoveryKey {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self> {
        let mut values = Zeroizing::new(Vec::with_capacity(SYMBOLS));
        for c in text.bytes() {
            if c == b'-' || c.is_ascii_whitespace() {
                continue;
            }
            let c = match c.to_ascii_uppercase() {
                b'I' | b'L' => b'1',
                b'O' => b'0',
                other => other,
            };
            let value = ALPHABET
                .iter()
                .position(|&a| a == c)
                .ok_or(Error::InvalidRecoveryKey)?;
            if values.len() == SYMBOLS {
                return Err(Error::InvalidRecoveryKey);
            }
            values.push(u16::try_from(value).map_err(|_| Error::InvalidRecoveryKey)?);
        }
        if values.len() != SYMBOLS {
            return Err(Error::InvalidRecoveryKey);
        }

        let mut key = Zeroizing::new([0u8; KEY_LEN]);
        let mut acc: u16 = 0;
        let mut bits = 0;
        let mut out = 0;
        for &value in values.iter() {
            acc = (acc << 5) | value;
            bits += 5;
            if bits >= 8 {
                bits -= 8;
                key[out] = u8::try_from(acc >> bits).map_err(|_| Error::InvalidRecoveryKey)?;
                out += 1;
                acc &= (1 << bits) - 1;
            }
        }
        if out != KEY_LEN || acc != 0 {
            return Err(Error::InvalidRecoveryKey);
        }
        Ok(Self(key))
    }
}

impl fmt::Debug for RecoveryKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RecoveryKey(..)")
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    proptest! {
        #[test]
        fn display_parse_roundtrip(bytes in any::<[u8; KEY_LEN]>()) {
            let key = RecoveryKey(Zeroizing::new(bytes));
            let text = key.to_display();
            prop_assert_eq!(text.len(), SYMBOLS + SYMBOLS.div_ceil(GROUP) - 1);
            prop_assert_eq!(*RecoveryKey::parse(&text).unwrap().0, bytes);
            let loose = text.to_lowercase().replace('-', " ").replace('0', "o");
            prop_assert_eq!(*loose.parse::<RecoveryKey>().unwrap().0, bytes);
        }
    }

    #[test]
    fn parse_rejects_bad_input() {
        let text = RecoveryKey::generate().unwrap().to_display();
        assert!(RecoveryKey::parse(&text[..text.len() - 1]).is_err());
        assert!(RecoveryKey::parse(&format!("{}0", &*text)).is_err());
        assert!(
            RecoveryKey::parse(&text.replacen(|c: char| c.is_ascii_alphanumeric(), "U", 1))
                .is_err()
        );
        let mut padded = text.to_string();
        padded.pop();
        padded.push('1');
        assert!(RecoveryKey::parse(&padded).is_err());
    }
}
