use zeroize::Zeroizing;

use crate::error::{Error, Result};

const LEN_PREFIX: usize = 8;

// Sized up front: a growing Vec would free reallocated buffers holding plaintext without wiping them.
pub(crate) fn encode(fields: &[&[u8]]) -> Zeroizing<Vec<u8>> {
    let total = fields.iter().map(|f| LEN_PREFIX + f.len()).sum();
    let mut out = Zeroizing::new(Vec::with_capacity(total));
    for field in fields {
        let len = u64::try_from(field.len()).unwrap_or(u64::MAX);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(field);
    }
    out
}

pub(crate) struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    pub(crate) fn new(bytes: &'a [u8]) -> Self {
        Self(bytes)
    }

    pub(crate) fn take(&mut self) -> Result<&'a [u8]> {
        let (len, rest) = self
            .0
            .split_first_chunk::<LEN_PREFIX>()
            .ok_or(Error::Corrupt)?;
        let len = usize::try_from(u64::from_le_bytes(*len)).map_err(|_| Error::Corrupt)?;
        let (field, rest) = rest.split_at_checked(len).ok_or(Error::Corrupt)?;
        self.0 = rest;
        Ok(field)
    }

    pub(crate) fn take_str(&mut self) -> Result<&'a str> {
        std::str::from_utf8(self.take()?).map_err(|_| Error::Corrupt)
    }

    pub(crate) fn take_u64(&mut self) -> Result<u64> {
        let bytes: [u8; 8] = self.take()?.try_into().map_err(|_| Error::Corrupt)?;
        Ok(u64::from_le_bytes(bytes))
    }

    pub(crate) fn finish(self) -> Result<()> {
        if self.0.is_empty() {
            Ok(())
        } else {
            Err(Error::Corrupt)
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn roundtrip_and_rejects_garbage() {
        let bytes = encode(&[b"name", b"", &42u64.to_le_bytes()]);
        assert_eq!(bytes.len(), bytes.capacity());
        let mut r = Reader::new(&bytes);
        assert_eq!(r.take_str().unwrap(), "name");
        assert_eq!(r.take().unwrap(), b"");
        assert_eq!(r.take_u64().unwrap(), 42);
        r.finish().unwrap();

        assert!(Reader::new(&bytes[..5]).take().is_err());
        let mut r = Reader::new(&bytes);
        r.take().unwrap();
        assert!(r.finish().is_err());
        let mut huge = u64::MAX.to_le_bytes().to_vec();
        huge.push(1);
        assert!(Reader::new(&huge).take().is_err());
    }
}
