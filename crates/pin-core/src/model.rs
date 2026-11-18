use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use zeroize::Zeroize;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntryId(pub(crate) i64);

impl EntryId {
    pub fn get(self) -> i64 {
        self.0
    }
}

impl fmt::Display for EntryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Timestamp(i64);

impl Timestamp {
    pub fn now() -> Self {
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
        Self(millis)
    }

    pub fn from_millis(millis: i64) -> Self {
        Self(millis)
    }

    pub fn as_millis(self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PinSlot(u8);

impl PinSlot {
    pub const MIN: u8 = 1;
    pub const MAX: u8 = 9;

    pub fn new(slot: u8) -> Result<Self> {
        if (Self::MIN..=Self::MAX).contains(&slot) {
            Ok(Self(slot))
        } else {
            Err(Error::InvalidSlot(slot))
        }
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FormatKind {
    Text,
    Html,
    Rtf,
    Png,
    Files,
    Other(String),
}

impl FormatKind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Text => "text/plain",
            Self::Html => "text/html",
            Self::Rtf => "text/rtf",
            Self::Png => "image/png",
            Self::Files => "text/uri-list",
            Self::Other(kind) => kind,
        }
    }

    pub fn parse(kind: &str) -> Self {
        match kind {
            "text/plain" => Self::Text,
            "text/html" => Self::Html,
            "text/rtf" => Self::Rtf,
            "image/png" => Self::Png,
            "text/uri-list" => Self::Files,
            other => Self::Other(other.to_owned()),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Format {
    pub kind: FormatKind,
    pub data: Vec<u8>,
}

impl Zeroize for Format {
    fn zeroize(&mut self) {
        self.data.zeroize();
    }
}

impl fmt::Debug for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Format")
            .field("kind", &self.kind)
            .field("len", &self.data.len())
            .finish()
    }
}

impl Format {
    pub fn new(kind: FormatKind, data: impl Into<Vec<u8>>) -> Self {
        Self {
            kind,
            data: data.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NewEntry {
    pub formats: Vec<Format>,
    pub source_app: Option<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct Entry {
    pub id: EntryId,
    pub kind: FormatKind,
    pub preview: String,
    pub source_app: Option<String>,
    pub size: u64,
    pub created_at: Timestamp,
    pub last_used_at: Timestamp,
    pub pinned: bool,
    pub slot: Option<PinSlot>,
}

impl fmt::Debug for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Entry")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .field("size", &self.size)
            .field("pinned", &self.pinned)
            .field("slot", &self.slot)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SecretId(pub(crate) i64);

impl SecretId {
    pub fn get(self) -> i64 {
        self.0
    }
}

impl fmt::Display for SecretId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

pub struct NewSecret {
    pub name: String,
    pub note: String,
    pub formats: Vec<Format>,
}

impl fmt::Debug for NewSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NewSecret")
            .field("formats", &self.formats)
            .finish_non_exhaustive()
    }
}

impl Drop for NewSecret {
    fn drop(&mut self) {
        self.name.zeroize();
        self.note.zeroize();
        self.formats.zeroize();
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct Secret {
    pub id: SecretId,
    pub name: String,
    pub note: String,
    pub kind: FormatKind,
    pub size: u64,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Secret")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        self.name.zeroize();
        self.note.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_never_contains_content() {
        let format = Format::new(FormatKind::Text, "hunter2");
        let secret = Secret {
            id: SecretId(1),
            name: "bank-login".into(),
            note: "pin-1234".into(),
            kind: FormatKind::Text,
            size: 7,
            created_at: Timestamp::from_millis(0),
            updated_at: Timestamp::from_millis(0),
        };
        let entry = Entry {
            id: EntryId(1),
            kind: FormatKind::Text,
            preview: "hunter2".into(),
            source_app: None,
            size: 7,
            created_at: Timestamp::from_millis(0),
            last_used_at: Timestamp::from_millis(0),
            pinned: false,
            slot: None,
        };
        let new = NewSecret {
            name: "bank-login".into(),
            note: String::new(),
            formats: vec![format.clone()],
        };
        let all = format!("{format:?} {secret:?} {entry:?} {new:?}");
        for leak in ["hunter2", "104, 117", "bank-login", "pin-1234"] {
            assert!(!all.contains(leak), "{leak} in {all}");
        }
    }
}
