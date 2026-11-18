use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Format {
    pub kind: FormatKind,
    pub data: Vec<u8>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
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
