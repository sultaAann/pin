use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use zeroize::Zeroize;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EntryId(pub(crate) i64);

impl EntryId {
    pub const fn get(self) -> i64 {
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

    pub const fn from_millis(millis: i64) -> Self {
        Self(millis)
    }

    pub const fn as_millis(self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PinSlot(u8);

impl PinSlot {
    pub const MIN: u8 = 1;
    pub const MAX: u8 = 9;

    pub fn new(slot: u8) -> Result<Self> {
        Self::try_from(slot)
    }

    pub const fn get(self) -> u8 {
        self.0
    }

    pub fn all() -> impl Iterator<Item = Self> {
        (Self::MIN..=Self::MAX).map(Self)
    }
}

impl TryFrom<u8> for PinSlot {
    type Error = Error;

    fn try_from(slot: u8) -> Result<Self> {
        if (Self::MIN..=Self::MAX).contains(&slot) {
            Ok(Self(slot))
        } else {
            Err(Error::InvalidSlot(slot))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pin {
    None,
    Pinned,
    Slot(PinSlot),
}

impl Pin {
    pub const fn is_pinned(self) -> bool {
        !matches!(self, Self::None)
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

    pub fn from_mime(kind: &str) -> Self {
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

impl From<&str> for FormatKind {
    fn from(kind: &str) -> Self {
        Self::from_mime(kind)
    }
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
    pub pin: Pin,
}

impl fmt::Debug for Entry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Entry")
            .field("id", &self.id)
            .field("kind", &self.kind)
            .field("size", &self.size)
            .field("pin", &self.pin)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_never_contains_content() {
        let format = Format::new(FormatKind::Text, "hunter2");
        let entry = Entry {
            id: EntryId(1),
            kind: FormatKind::Text,
            preview: "hunter2".into(),
            source_app: None,
            size: 7,
            created_at: Timestamp::from_millis(0),
            last_used_at: Timestamp::from_millis(0),
            pin: Pin::None,
        };
        let all = format!("{format:?} {entry:?}");
        for leak in ["hunter2", "104, 117"] {
            assert!(!all.contains(leak), "{leak} in {all}");
        }
    }

    #[test]
    fn pin_slot_converts_only_in_range() {
        assert_eq!(PinSlot::try_from(PinSlot::MIN).unwrap().get(), PinSlot::MIN);
        assert!(PinSlot::try_from(PinSlot::MAX + 1).is_err());
        assert!(PinSlot::try_from(0).is_err());
        let all: Vec<u8> = PinSlot::all().map(PinSlot::get).collect();
        assert_eq!(all, (PinSlot::MIN..=PinSlot::MAX).collect::<Vec<_>>());
    }

    #[test]
    fn format_kind_roundtrips_through_mime() {
        for kind in [
            FormatKind::Text,
            FormatKind::Html,
            FormatKind::Rtf,
            FormatKind::Png,
            FormatKind::Files,
            FormatKind::Other("x/y".into()),
        ] {
            assert_eq!(FormatKind::from(kind.as_str()), kind);
        }
    }
}
