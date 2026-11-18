use std::mem;

use rusqlite::{Connection, OptionalExtension, params};
use zeroize::{Zeroize, Zeroizing};

use super::codec::{Reader, encode};
use super::session::{Session, new_uid, session, uid_from};
use super::{Vault, session::UID};
use crate::content;
use crate::error::{Error, Result};
use crate::model::{EntryId, Format, FormatKind, NewSecret, Secret, SecretId, Timestamp};
use crate::store::{checkpoint, delete_entry, load_formats, write_tx};

const AAD_SECRET_META: &[u8] = b"pin/secret/meta";
const AAD_SECRET_FORMAT: &[u8] = b"pin/secret/format";

struct MetaRef<'a> {
    name: &'a str,
    note: &'a str,
    kind: &'a FormatKind,
    size: u64,
    formats: u64,
}

struct Meta {
    name: String,
    note: String,
    kind: FormatKind,
    size: u64,
    formats: u64,
}

impl Drop for Meta {
    fn drop(&mut self) {
        self.name.zeroize();
        self.note.zeroize();
    }
}

impl Vault<'_> {
    pub fn add(&mut self, mut new: NewSecret) -> Result<SecretId> {
        let tx = write_tx(self.conn)?;
        let session = session(self.session, &tx, self.options.auto_lock)?;
        let id = insert(
            &tx,
            session,
            &new.name,
            &new.note,
            mem::take(&mut new.formats),
        )?;
        tx.commit()?;
        Ok(id)
    }

    pub fn move_from_history(
        &mut self,
        entry: EntryId,
        name: String,
        note: String,
    ) -> Result<SecretId> {
        let name = Zeroizing::new(name);
        let note = Zeroizing::new(note);
        let tx = write_tx(self.conn)?;
        let session = session(self.session, &tx, self.options.auto_lock)?;
        let formats = load_formats(&tx, entry)?;
        let id = insert(&tx, session, &name, &note, formats)?;
        delete_entry(&tx, entry)?;
        tx.commit()?;
        checkpoint(self.conn);
        Ok(id)
    }

    pub fn secrets(&mut self) -> Result<Vec<Secret>> {
        let tx = self.conn.transaction()?;
        let session = session(self.session, &tx, self.options.auto_lock)?;
        let rows = {
            let mut stmt =
                tx.prepare_cached("SELECT id, uid, meta, created_at, updated_at FROM secrets")?;
            stmt.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, Vec<u8>>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
        };
        tx.commit()?;
        let mut secrets = rows
            .into_iter()
            .map(|(id, uid, meta, created_at, updated_at)| {
                let mut meta = open_meta(session, &uid_from(&uid)?, &meta)?;
                Ok(Secret {
                    id: SecretId(id),
                    name: mem::take(&mut meta.name),
                    note: mem::take(&mut meta.note),
                    kind: mem::replace(&mut meta.kind, FormatKind::Text),
                    size: meta.size,
                    created_at: Timestamp::from_millis(created_at),
                    updated_at: Timestamp::from_millis(updated_at),
                })
            })
            .collect::<Result<Vec<_>>>()?;
        secrets.sort_by(|a, b| {
            a.name
                .chars()
                .flat_map(char::to_lowercase)
                .cmp(b.name.chars().flat_map(char::to_lowercase))
                .then(a.id.cmp(&b.id))
        });
        Ok(secrets)
    }

    pub fn formats(&mut self, id: SecretId) -> Result<Zeroizing<Vec<Format>>> {
        let tx = self.conn.transaction()?;
        let session = session(self.session, &tx, self.options.auto_lock)?;
        let (uid, meta) = load_meta(&tx, session, id)?;
        let sealed = {
            let mut stmt = tx.prepare_cached(
                "SELECT data FROM secret_formats WHERE secret_id = ?1 ORDER BY id",
            )?;
            stmt.query_map([id.0], |row| row.get::<_, Vec<u8>>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?
        };
        tx.commit()?;
        if u64::try_from(sealed.len()).ok() != Some(meta.formats) {
            return Err(Error::Corrupt);
        }
        let mut formats = Zeroizing::new(Vec::with_capacity(sealed.len()));
        for (index, data) in sealed.iter().enumerate() {
            let plain = open_format(session, &uid, index, sealed.len(), data)?;
            let mut reader = Reader::new(&plain);
            let kind = FormatKind::parse(reader.take_str()?);
            let data = reader.take()?.to_vec();
            reader.finish()?;
            formats.push(Format { kind, data });
        }
        Ok(formats)
    }

    pub fn update(&mut self, id: SecretId, name: String, note: String) -> Result<()> {
        let name = Zeroizing::new(name);
        let note = Zeroizing::new(note);
        let tx = write_tx(self.conn)?;
        let session = session(self.session, &tx, self.options.auto_lock)?;
        let (uid, old) = load_meta(&tx, session, id)?;
        let meta = MetaRef {
            name: checked_name(&name)?,
            note: &note,
            kind: &old.kind,
            size: old.size,
            formats: old.formats,
        };
        tx.execute(
            "UPDATE secrets SET meta = ?2, updated_at = ?3 WHERE id = ?1",
            params![
                id.0,
                seal_meta(session, &uid, &meta)?,
                Timestamp::now().as_millis()
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn delete(&mut self, id: SecretId) -> Result<()> {
        let tx = write_tx(self.conn)?;
        session(self.session, &tx, self.options.auto_lock)?;
        if tx.execute("DELETE FROM secrets WHERE id = ?1", [id.0])? == 0 {
            return Err(Error::SecretNotFound(id));
        }
        tx.commit()?;
        Ok(())
    }
}

fn insert(
    conn: &Connection,
    session: &Session,
    name: &str,
    note: &str,
    formats: Vec<Format>,
) -> Result<SecretId> {
    let formats = Zeroizing::new(content::normalize(formats));
    let name = checked_name(name)?;
    if formats.is_empty() {
        return Err(Error::Empty);
    }
    let uid = new_uid()?;
    let kind = content::primary_kind(&formats);
    let meta = MetaRef {
        name,
        note,
        kind: &kind,
        size: u64::try_from(content::size(&formats)).unwrap_or(u64::MAX),
        formats: u64::try_from(formats.len()).unwrap_or(u64::MAX),
    };
    let now = Timestamp::now().as_millis();
    conn.execute(
        "INSERT INTO secrets (uid, meta, created_at, updated_at) VALUES (?1, ?2, ?3, ?3)",
        params![&uid[..], seal_meta(session, &uid, &meta)?, now],
    )?;
    let id = conn.last_insert_rowid();
    let mut stmt = conn.prepare("INSERT INTO secret_formats (secret_id, data) VALUES (?1, ?2)")?;
    for (index, format) in formats.iter().enumerate() {
        let plain = encode(&[format.kind.as_str().as_bytes(), &format.data]);
        stmt.execute(params![
            id,
            seal_format(session, &uid, index, formats.len(), &plain)?
        ])?;
    }
    Ok(SecretId(id))
}

// Index and count bind each format to its position, so rows cannot be reordered or duplicated
// without failing decryption; the count sealed in the meta catches dropped rows.
fn position(index: usize, count: usize) -> [[u8; 8]; 2] {
    let as_bytes = |n: usize| u64::try_from(n).unwrap_or(u64::MAX).to_le_bytes();
    [as_bytes(index), as_bytes(count)]
}

fn seal_format(
    session: &Session,
    uid: &[u8; UID],
    index: usize,
    count: usize,
    plain: &[u8],
) -> Result<Vec<u8>> {
    let [index, count] = position(index, count);
    session.seal(AAD_SECRET_FORMAT, &[uid, &index, &count], plain)
}

fn open_format(
    session: &Session,
    uid: &[u8; UID],
    index: usize,
    count: usize,
    sealed: &[u8],
) -> Result<Zeroizing<Vec<u8>>> {
    let [index, count] = position(index, count);
    session.open(AAD_SECRET_FORMAT, &[uid, &index, &count], sealed)
}

fn seal_meta(session: &Session, uid: &[u8; UID], meta: &MetaRef<'_>) -> Result<Vec<u8>> {
    let plain = encode(&[
        meta.name.as_bytes(),
        meta.note.as_bytes(),
        meta.kind.as_str().as_bytes(),
        &meta.size.to_le_bytes(),
        &meta.formats.to_le_bytes(),
    ]);
    session.seal(AAD_SECRET_META, &[uid], &plain)
}

fn open_meta(session: &Session, uid: &[u8; UID], sealed: &[u8]) -> Result<Meta> {
    let plain = session.open(AAD_SECRET_META, &[uid], sealed)?;
    let mut reader = Reader::new(&plain);
    let meta = Meta {
        name: reader.take_str()?.to_owned(),
        note: reader.take_str()?.to_owned(),
        kind: FormatKind::parse(reader.take_str()?),
        size: reader.take_u64()?,
        formats: reader.take_u64()?,
    };
    reader.finish()?;
    Ok(meta)
}

fn load_meta(conn: &Connection, session: &Session, id: SecretId) -> Result<([u8; UID], Meta)> {
    let (uid, sealed): (Vec<u8>, Vec<u8>) = conn
        .query_row(
            "SELECT uid, meta FROM secrets WHERE id = ?1",
            [id.0],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?
        .ok_or(Error::SecretNotFound(id))?;
    let uid = uid_from(&uid)?;
    let meta = open_meta(session, &uid, &sealed)?;
    Ok((uid, meta))
}

fn checked_name(name: &str) -> Result<&str> {
    if name.trim().is_empty() {
        Err(Error::EmptyName)
    } else {
        Ok(name)
    }
}
