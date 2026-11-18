use crate::model::EntryId;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("wrong database key or not a pin database")]
    WrongKey,
    #[error("database schema version {found} is newer than supported {supported}")]
    UnsupportedSchema { found: i64, supported: i64 },
    #[error("entry {0} not found")]
    NotFound(EntryId),
    #[error("pin slot {0} is out of range 1..=9")]
    InvalidSlot(u8),
    #[error("entry has no formats")]
    Empty,
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
