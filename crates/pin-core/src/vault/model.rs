use std::fmt;

use zeroize::Zeroize;

use crate::model::{Format, FormatKind, Timestamp};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SecretId(pub(crate) i64);

impl SecretId {
    pub const fn get(self) -> i64 {
        self.0
    }
}

impl fmt::Display for SecretId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

pub struct SecretLabel {
    pub name: String,
    pub note: String,
}

impl fmt::Debug for SecretLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SecretLabel").finish_non_exhaustive()
    }
}

impl Drop for SecretLabel {
    fn drop(&mut self) {
        self.name.zeroize();
        self.note.zeroize();
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
        let secret = Secret {
            id: SecretId(1),
            name: "bank-login".into(),
            note: "pin-1234".into(),
            kind: FormatKind::Text,
            size: 7,
            created_at: Timestamp::from_millis(0),
            updated_at: Timestamp::from_millis(0),
        };
        let new = NewSecret {
            name: "bank-login".into(),
            note: String::new(),
            formats: vec![Format::new(FormatKind::Text, "hunter2")],
        };
        let label = SecretLabel {
            name: "bank-login".into(),
            note: "pin-1234".into(),
        };
        let all = format!("{secret:?} {new:?} {label:?}");
        for leak in ["hunter2", "104, 117", "bank-login", "pin-1234"] {
            assert!(!all.contains(leak), "{leak} in {all}");
        }
    }
}
