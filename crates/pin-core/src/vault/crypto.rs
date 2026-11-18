use std::fmt;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use zeroize::Zeroizing;

use super::KdfParams;
use super::codec::encode;
use crate::error::{Error, Result};
use crate::random;

pub(crate) const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 24;

const MAX_MEMORY_KIB: u32 = 1024 * 1024;
const MAX_ITERATIONS: u32 = 64;
const MAX_PARALLELISM: u32 = 16;

pub(crate) struct SecretKey(Zeroizing<[u8; KEY_LEN]>);

impl SecretKey {
    pub(crate) fn random() -> Result<Self> {
        let mut key = Zeroizing::new([0u8; KEY_LEN]);
        random::fill(key.as_mut_slice())?;
        Ok(Self(key))
    }

    pub(crate) fn from_slice(bytes: &[u8]) -> Result<Self> {
        let mut key = Zeroizing::new([0u8; KEY_LEN]);
        if bytes.len() != KEY_LEN {
            return Err(Error::Corrupt);
        }
        key.copy_from_slice(bytes);
        Ok(Self(key))
    }

    pub(crate) fn derived(context: &str, material: &[u8]) -> Self {
        Self(Zeroizing::new(blake3::derive_key(context, material)))
    }

    pub(crate) fn as_bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }
}

impl fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SecretKey(..)")
    }
}

pub(crate) fn derive(password: &[u8], salt: &[u8], params: KdfParams) -> Result<SecretKey> {
    if params.memory_kib > MAX_MEMORY_KIB
        || !(1..=MAX_ITERATIONS).contains(&params.iterations)
        || !(1..=MAX_PARALLELISM).contains(&params.parallelism)
    {
        return Err(Error::InvalidKdfParams);
    }
    let params = argon2::Params::new(
        params.memory_kib,
        params.iterations,
        params.parallelism,
        Some(KEY_LEN),
    )
    .map_err(|_| Error::InvalidKdfParams)?;
    let argon = argon2::Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    argon
        .hash_password_into(password, salt, key.as_mut_slice())
        .map_err(|_| Error::InvalidKdfParams)?;
    Ok(SecretKey(key))
}

pub(crate) fn seal(key: &SecretKey, aad: &[&[u8]], plaintext: &[u8]) -> Result<Vec<u8>> {
    let nonce: [u8; NONCE_LEN] = random::array()?;
    let aad = encode(aad);
    let ciphertext = cipher(key)
        .encrypt(
            &XNonce::from(nonce),
            Payload {
                msg: plaintext,
                aad: &aad,
            },
        )
        .map_err(|_| Error::Corrupt)?;
    let mut sealed = Vec::with_capacity(NONCE_LEN + ciphertext.len());
    sealed.extend_from_slice(&nonce);
    sealed.extend_from_slice(&ciphertext);
    Ok(sealed)
}

pub(crate) fn open(key: &SecretKey, aad: &[&[u8]], sealed: &[u8]) -> Result<Zeroizing<Vec<u8>>> {
    let (nonce, ciphertext) = sealed
        .split_first_chunk::<NONCE_LEN>()
        .ok_or(Error::Corrupt)?;
    let aad = encode(aad);
    cipher(key)
        .decrypt(
            &XNonce::from(*nonce),
            Payload {
                msg: ciphertext,
                aad: &aad,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| Error::Corrupt)
}

fn cipher(key: &SecretKey) -> XChaCha20Poly1305 {
    XChaCha20Poly1305::new(key.as_bytes().into())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    const FAST: KdfParams = KdfParams {
        memory_kib: 64,
        iterations: 1,
        parallelism: 1,
    };

    #[test]
    fn sealed_data_opens_only_with_same_key_and_aad() {
        let key = SecretKey::random().unwrap();
        let sealed = seal(&key, &[b"aad"], b"secret").unwrap();
        assert_eq!(
            open(&key, &[b"aad"], &sealed).unwrap().as_slice(),
            b"secret"
        );
        assert!(matches!(
            open(&key, &[b"other"], &sealed),
            Err(Error::Corrupt)
        ));
        assert!(matches!(
            open(&key, &[b"a", b"ad"], &sealed),
            Err(Error::Corrupt)
        ));
        let mut flipped = sealed.clone();
        *flipped.last_mut().unwrap() ^= 1;
        assert!(matches!(
            open(&key, &[b"aad"], &flipped),
            Err(Error::Corrupt)
        ));
        assert!(matches!(
            open(&key, &[b"aad"], &sealed[..10]),
            Err(Error::Corrupt)
        ));
        let other = SecretKey::random().unwrap();
        assert!(matches!(
            open(&other, &[b"aad"], &sealed),
            Err(Error::Corrupt)
        ));
    }

    #[test]
    fn derived_key_depends_on_password_and_salt() {
        let a = derive(b"pw", b"saltsaltsaltsalt", FAST).unwrap();
        let same = derive(b"pw", b"saltsaltsaltsalt", FAST).unwrap();
        assert_eq!(a.as_bytes(), same.as_bytes());
        assert_ne!(
            a.as_bytes(),
            derive(b"pw2", b"saltsaltsaltsalt", FAST)
                .unwrap()
                .as_bytes()
        );
        assert_ne!(
            a.as_bytes(),
            derive(b"pw", b"saltsaltsaltsal2", FAST).unwrap().as_bytes()
        );
    }

    #[test]
    fn oversized_kdf_params_are_rejected() {
        let huge = KdfParams {
            memory_kib: MAX_MEMORY_KIB + 1,
            ..FAST
        };
        assert!(matches!(
            derive(b"pw", b"saltsaltsaltsalt", huge),
            Err(Error::InvalidKdfParams)
        ));
        let zero = KdfParams {
            iterations: 0,
            ..FAST
        };
        assert!(matches!(
            derive(b"pw", b"saltsaltsaltsalt", zero),
            Err(Error::InvalidKdfParams)
        ));
    }

    #[test]
    fn secret_key_debug_is_redacted() {
        assert_eq!(
            format!("{:?}", SecretKey::random().unwrap()),
            "SecretKey(..)"
        );
    }
}
