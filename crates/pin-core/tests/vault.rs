#![allow(clippy::unwrap_used)]

use std::cell::RefCell;
use std::fmt::Write as _;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use pin_core::{
    Error, Format, FormatKind, KdfParams, Key, KeyStore, MasterPassword, NewEntry, NewSecret,
    Options, PinSlot, RecoveryKey, Store, VaultOptions, VaultStatus,
};

fn key() -> Key {
    Key::from_bytes(&mut [7; Key::LEN])
}

fn pw(s: &str) -> MasterPassword {
    MasterPassword::new(s.into()).unwrap()
}

fn options() -> Options {
    Options {
        vault: VaultOptions {
            kdf: KdfParams {
                memory_kib: 64,
                iterations: 1,
                parallelism: 1,
            },
            ..VaultOptions::default()
        },
        ..Options::default()
    }
}

fn open(dir: &Path) -> Store {
    open_with(dir, options())
}

fn open_with(dir: &Path, options: Options) -> Store {
    Store::open(&dir.join("pin.db"), &key(), options).unwrap()
}

fn created(dir: &Path) -> Store {
    let mut store = open(dir);
    let _recovery = store.vault().create(&pw("correct horse")).unwrap();
    store
}

fn secret(name: &str, value: &str) -> NewSecret {
    NewSecret {
        name: name.into(),
        note: String::new(),
        formats: vec![Format::new(FormatKind::Text, value)],
    }
}

fn names(store: &mut Store) -> Vec<String> {
    store
        .vault()
        .secrets()
        .unwrap()
        .iter()
        .map(|s| s.name.clone())
        .collect()
}

fn raw(dir: &Path) -> rusqlite::Connection {
    let conn = rusqlite::Connection::open(dir.join("pin.db")).unwrap();
    let hex = key().as_bytes().iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    });
    conn.execute_batch(&format!("PRAGMA key = \"x'{hex}'\";"))
        .unwrap();
    conn
}

#[test]
fn new_database_has_no_vault() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path());
    assert_eq!(store.vault().status().unwrap(), VaultStatus::Missing);
    assert!(matches!(
        store.vault().unlock(&pw("x")),
        Err(Error::VaultMissing)
    ));
    assert!(matches!(store.vault().secrets(), Err(Error::VaultLocked)));
}

#[test]
fn empty_master_password_is_rejected() {
    assert!(matches!(
        MasterPassword::new(String::new()),
        Err(Error::EmptyPassword)
    ));
    assert_eq!(format!("{:?}", pw("secret")), "MasterPassword(..)");
}

#[test]
fn created_vault_is_unlocked_and_cannot_be_created_twice() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = created(dir.path());
    assert_eq!(store.vault().status().unwrap(), VaultStatus::Unlocked);
    assert!(matches!(
        store.vault().create(&pw("other")),
        Err(Error::VaultExists)
    ));
    let mut other = open(dir.path());
    assert!(matches!(
        other.vault().create(&pw("other")),
        Err(Error::VaultExists)
    ));
}

#[test]
fn reopened_store_starts_locked_and_unlocks_with_password() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = created(dir.path());
    store.vault().add(secret("token", "s3cr3t")).unwrap();
    drop(store);

    let mut store = open(dir.path());
    assert_eq!(store.vault().status().unwrap(), VaultStatus::Locked);
    assert!(matches!(store.vault().secrets(), Err(Error::VaultLocked)));
    assert!(matches!(
        store.vault().unlock(&pw("wrong")),
        Err(Error::WrongPassword)
    ));
    store.vault().unlock(&pw("correct horse")).unwrap();
    assert_eq!(names(&mut store), ["token"]);
}

#[test]
fn locked_vault_rejects_every_secret_operation() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = created(dir.path());
    let id = store.vault().add(secret("a", "1")).unwrap();
    store.vault().lock();
    assert_eq!(store.vault().status().unwrap(), VaultStatus::Locked);
    let mut vault = store.vault();
    assert!(matches!(
        vault.add(secret("x", "y")),
        Err(Error::VaultLocked)
    ));
    assert!(matches!(vault.formats(id), Err(Error::VaultLocked)));
    assert!(matches!(
        vault.update(id, "n".into(), String::new()),
        Err(Error::VaultLocked)
    ));
    assert!(matches!(vault.delete(id), Err(Error::VaultLocked)));
}

#[test]
fn secret_roundtrips_all_formats_and_metadata() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = created(dir.path());
    let formats = vec![
        Format::new(FormatKind::Text, "hunter2"),
        Format::new(FormatKind::Html, "<b>hunter2</b>"),
    ];
    let id = store
        .vault()
        .add(NewSecret {
            name: "bank".into(),
            note: "pin code".into(),
            formats: formats.clone(),
        })
        .unwrap();

    let listed = store.vault().secrets().unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, id);
    assert_eq!(listed[0].note, "pin code");
    assert_eq!(listed[0].kind, FormatKind::Text);
    assert_eq!(listed[0].size, 7 + 14);

    let mut stored = store.vault().formats(id).unwrap().to_vec();
    stored.sort_by(|x, y| x.kind.cmp(&y.kind));
    let mut expected = formats;
    expected.sort_by(|x, y| x.kind.cmp(&y.kind));
    assert_eq!(stored, expected);
}

#[test]
fn secrets_are_sorted_by_name_ignoring_case() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = created(dir.path());
    for name in ["bank", "Api key", "Ёлка", "apple"] {
        store.vault().add(secret(name, "v")).unwrap();
    }
    assert_eq!(names(&mut store), ["Api key", "apple", "bank", "Ёлка"]);
}

#[test]
fn update_changes_name_and_note_but_keeps_value() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = created(dir.path());
    let id = store.vault().add(secret("old", "abc")).unwrap();
    store
        .vault()
        .update(id, "new".into(), "rotated".into())
        .unwrap();
    let listed = store.vault().secrets().unwrap();
    assert_eq!(
        (listed[0].name.as_str(), listed[0].note.as_str()),
        ("new", "rotated")
    );
    assert_eq!(store.vault().formats(id).unwrap()[0].data, b"abc");
    assert!(matches!(
        store.vault().update(id, " ".into(), String::new()),
        Err(Error::EmptyName)
    ));
}

#[test]
fn deleted_secret_is_gone() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = created(dir.path());
    let id = store.vault().add(secret("a", "1")).unwrap();
    store.vault().delete(id).unwrap();
    assert!(names(&mut store).is_empty());
    let mut vault = store.vault();
    assert!(matches!(vault.delete(id), Err(Error::SecretNotFound(_))));
    assert!(matches!(vault.formats(id), Err(Error::SecretNotFound(_))));
    assert!(matches!(
        vault.update(id, "n".into(), String::new()),
        Err(Error::SecretNotFound(_))
    ));
}

#[test]
fn invalid_new_secrets_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = created(dir.path());
    assert!(matches!(
        store.vault().add(secret("  ", "v")),
        Err(Error::EmptyName)
    ));
    let empty = NewSecret {
        name: "n".into(),
        note: String::new(),
        formats: vec![],
    };
    assert!(matches!(store.vault().add(empty), Err(Error::Empty)));
}

#[test]
fn old_password_is_rejected_after_change() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = created(dir.path());
    store.vault().add(secret("a", "1")).unwrap();
    assert!(matches!(
        store.vault().change_password(&pw("nope"), &pw("new")),
        Err(Error::WrongPassword)
    ));
    store
        .vault()
        .change_password(&pw("correct horse"), &pw("new"))
        .unwrap();
    store.vault().lock();
    assert!(matches!(
        store.vault().unlock(&pw("correct horse")),
        Err(Error::WrongPassword)
    ));
    store.vault().unlock(&pw("new")).unwrap();
    assert_eq!(names(&mut store), ["a"]);
}

#[test]
fn password_change_from_stale_read_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = created(dir.path());
    let mut cli = open(dir.path());
    cli.vault().unlock(&pw("correct horse")).unwrap();
    app.vault()
        .change_password(&pw("correct horse"), &pw("first"))
        .unwrap();
    assert!(matches!(
        cli.vault()
            .change_password(&pw("correct horse"), &pw("second")),
        Err(Error::WrongPassword)
    ));
    app.vault().lock();
    app.vault().unlock(&pw("first")).unwrap();
}

#[test]
fn recovery_sets_new_password_and_rotates_recovery_key() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path());
    let text = store
        .vault()
        .create(&pw("correct horse"))
        .unwrap()
        .to_display()
        .to_string();
    store.vault().add(secret("a", "1")).unwrap();
    drop(store);

    let mut store = open(dir.path());
    let parsed = RecoveryKey::parse(&text.to_lowercase()).unwrap();
    let rotated = store.vault().recover(&parsed, &pw("fresh")).unwrap();
    assert_ne!(*rotated.to_display(), text);
    assert_eq!(store.vault().status().unwrap(), VaultStatus::Unlocked);
    assert_eq!(names(&mut store), ["a"]);

    store.vault().lock();
    assert!(matches!(
        store.vault().unlock(&pw("correct horse")),
        Err(Error::WrongPassword)
    ));
    store.vault().unlock(&pw("fresh")).unwrap();
    store.vault().lock();
    assert!(matches!(
        store.vault().recover(&parsed, &pw("again")),
        Err(Error::InvalidRecoveryKey)
    ));
    let _next = store.vault().recover(&rotated, &pw("again")).unwrap();
}

#[test]
fn foreign_or_malformed_recovery_key_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = created(dir.path());
    let other_dir = tempfile::tempdir().unwrap();
    let foreign = open(other_dir.path()).vault().create(&pw("x")).unwrap();
    assert!(matches!(
        store.vault().recover(&foreign, &pw("fresh")),
        Err(Error::InvalidRecoveryKey)
    ));
    assert!(matches!(
        RecoveryKey::parse("not a key"),
        Err(Error::InvalidRecoveryKey)
    ));
}

#[test]
fn moved_entry_leaves_history_and_search() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path());
    let entry = store
        .insert(NewEntry {
            formats: vec![Format::new(FormatKind::Text, "password123")],
            source_app: None,
        })
        .unwrap();
    assert!(matches!(
        store
            .vault()
            .move_from_history(entry, "pw".into(), String::new()),
        Err(Error::VaultLocked)
    ));
    assert!(store.get(entry).is_ok());

    let _recovery = store.vault().create(&pw("correct horse")).unwrap();
    let id = store
        .vault()
        .move_from_history(entry, "pw".into(), "note".into())
        .unwrap();
    assert!(matches!(store.get(entry), Err(Error::NotFound(_))));
    assert!(store.search("password", 10).unwrap().is_empty());
    assert!(store.recent(10, 0).unwrap().is_empty());
    assert_eq!(store.vault().formats(id).unwrap()[0].data, b"password123");
    assert!(matches!(
        store
            .vault()
            .move_from_history(entry, "pw".into(), String::new()),
        Err(Error::NotFound(_))
    ));
}

#[test]
fn moved_pinned_entry_keeps_all_formats_and_frees_slot() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = created(dir.path());
    let entry = store
        .insert(NewEntry {
            formats: vec![
                Format::new(FormatKind::Text, "card 4242"),
                Format::new(FormatKind::Rtf, "{\\rtf1 card 4242}"),
            ],
            source_app: Some("Notes".into()),
        })
        .unwrap();
    let slot = PinSlot::new(3).unwrap();
    store.set_slot(entry, Some(slot)).unwrap();
    let id = store
        .vault()
        .move_from_history(entry, "card".into(), String::new())
        .unwrap();
    assert!(store.by_slot(slot).unwrap().is_none());
    let mut kinds: Vec<_> = store
        .vault()
        .formats(id)
        .unwrap()
        .iter()
        .map(|f| f.kind.clone())
        .collect();
    kinds.sort();
    assert_eq!(kinds, [FormatKind::Text, FormatKind::Rtf]);
}

#[test]
fn vault_locks_after_idle_timeout() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open_with(
        dir.path(),
        Options {
            vault: VaultOptions {
                auto_lock: Some(Duration::from_millis(400)),
                ..options().vault
            },
            ..options()
        },
    );
    let _recovery = store.vault().create(&pw("correct horse")).unwrap();
    assert!(store.vault().lock_deadline().is_some());
    std::thread::sleep(Duration::from_millis(150));
    store.vault().add(secret("a", "1")).unwrap();
    std::thread::sleep(Duration::from_millis(150));
    assert_eq!(store.vault().status().unwrap(), VaultStatus::Unlocked);
    std::thread::sleep(Duration::from_millis(600));
    assert!(store.vault().lock_if_idle());
    assert!(store.vault().lock_deadline().is_none());
    assert_eq!(store.vault().status().unwrap(), VaultStatus::Locked);
    assert!(matches!(store.vault().secrets(), Err(Error::VaultLocked)));
}

#[test]
fn reset_by_other_handle_locks_this_one() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = created(dir.path());
    let mut cli = open(dir.path());
    cli.vault().unlock(&pw("correct horse")).unwrap();
    app.vault().add(secret("a", "1")).unwrap();
    assert_eq!(names(&mut cli), ["a"]);

    app.vault().reset().unwrap();
    assert_eq!(app.vault().status().unwrap(), VaultStatus::Missing);
    assert_eq!(cli.vault().status().unwrap(), VaultStatus::Missing);
    let _recovery = app.vault().create(&pw("other")).unwrap();
    assert_eq!(cli.vault().status().unwrap(), VaultStatus::Locked);
    assert!(matches!(
        cli.vault().add(secret("b", "2")),
        Err(Error::VaultLocked)
    ));
    assert!(names(&mut app).is_empty());
}

#[test]
fn database_key_alone_does_not_reveal_secrets() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = created(dir.path());
    store
        .vault()
        .add(NewSecret {
            name: "vault-name-marker".into(),
            note: "vault-note-marker".into(),
            formats: vec![Format::new(FormatKind::Text, "vault-value-marker")],
        })
        .unwrap();
    drop(store);

    let conn = raw(dir.path());
    let mut blobs: Vec<Vec<u8>> = Vec::new();
    for sql in [
        "SELECT meta FROM secrets",
        "SELECT data FROM secret_formats",
        "SELECT CAST(text AS BLOB) FROM entries_fts",
        "SELECT CAST(preview AS BLOB) FROM entries",
    ] {
        let mut stmt = conn.prepare(sql).unwrap();
        blobs.extend(
            stmt.query_map([], |r| r.get(0))
                .unwrap()
                .map(Result::unwrap),
        );
    }
    assert!(!blobs.is_empty());
    for blob in blobs {
        for marker in [
            "vault-name-marker",
            "vault-note-marker",
            "vault-value-marker",
        ] {
            assert!(!blob.windows(marker.len()).any(|w| w == marker.as_bytes()));
        }
    }
}

#[test]
fn tampered_rows_fail_to_decrypt() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = created(dir.path());
    let a = store
        .vault()
        .add(NewSecret {
            name: "a".into(),
            note: String::new(),
            formats: vec![
                Format::new(FormatKind::Text, "one"),
                Format::new(FormatKind::Html, "<i>one</i>"),
            ],
        })
        .unwrap();
    let b = store.vault().add(secret("b", "two")).unwrap();
    let conn = raw(dir.path());

    conn.execute_batch("UPDATE secret_formats SET data = (SELECT data FROM secret_formats WHERE id = 3) WHERE id = 1")
        .unwrap();
    assert!(matches!(store.vault().formats(a), Err(Error::Corrupt)));

    conn.execute_batch(
        "DELETE FROM secret_formats WHERE id = 1;
         UPDATE secret_formats SET secret_id = (SELECT secret_id FROM secret_formats WHERE id = 3)
         WHERE id = 2",
    )
    .unwrap();
    assert!(matches!(store.vault().formats(a), Err(Error::Corrupt)));
    assert!(matches!(store.vault().formats(b), Err(Error::Corrupt)));

    conn.execute(
        "UPDATE secrets SET meta = (SELECT meta FROM secrets WHERE id = ?2) WHERE id = ?1",
        [a.get(), b.get()],
    )
    .unwrap();
    assert!(matches!(store.vault().secrets(), Err(Error::Corrupt)));
}

#[test]
fn upgrade_from_v1_keeps_history_and_scrubs_search_index() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path());
    let kept = store
        .insert(NewEntry {
            formats: vec![Format::new(FormatKind::Text, "kept entry")],
            source_app: None,
        })
        .unwrap();
    drop(store);
    raw(dir.path())
        .execute_batch(
            "DROP TABLE secret_formats; DROP TABLE secrets; DROP TABLE vault;
             INSERT INTO entries_fts (entries_fts, rank) VALUES ('secure-delete', 0);
             PRAGMA user_version = 1;",
        )
        .unwrap();

    let mut store = open(dir.path());
    assert_eq!(store.get(kept).unwrap().preview, "kept entry");
    assert_eq!(store.vault().status().unwrap(), VaultStatus::Missing);
    let conn = raw(dir.path());
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    let secure: i64 = conn
        .query_row(
            "SELECT v FROM entries_fts_config WHERE k = 'secure-delete'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!((version, secure), (2, 1));
}

#[derive(Default)]
struct MemoryKeys(RefCell<Option<Vec<u8>>>);

impl KeyStore for MemoryKeys {
    fn load(&self) -> pin_core::Result<Option<Key>> {
        self.0.borrow().as_deref().map(Key::from_slice).transpose()
    }

    fn save(&self, key: &Key) -> pin_core::Result<()> {
        *self.0.borrow_mut() = Some(key.as_bytes().to_vec());
        Ok(())
    }
}

#[test]
fn open_with_key_store_creates_key_once_and_refuses_lost_key() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pin.db");
    let keys = MemoryKeys::default();
    let mut store = Store::open_with(&path, &keys, options()).unwrap();
    store
        .insert(NewEntry {
            formats: vec![Format::new(FormatKind::Text, "hello")],
            source_app: None,
        })
        .unwrap();
    drop(store);

    let store = Store::open_with(&path, &keys, options()).unwrap();
    assert_eq!(store.recent(10, 0).unwrap().len(), 1);
    drop(store);

    assert!(matches!(
        Store::open_with(&path, &MemoryKeys::default(), options()),
        Err(Error::KeyMissing)
    ));
}

#[derive(Default)]
struct SlowKeys {
    key: Mutex<Option<Vec<u8>>>,
    saves: Mutex<usize>,
}

impl KeyStore for SlowKeys {
    fn load(&self) -> pin_core::Result<Option<Key>> {
        let stored = self.key.lock().unwrap().clone();
        std::thread::sleep(Duration::from_millis(50));
        stored.as_deref().map(Key::from_slice).transpose()
    }

    fn save(&self, key: &Key) -> pin_core::Result<()> {
        *self.saves.lock().unwrap() += 1;
        *self.key.lock().unwrap() = Some(key.as_bytes().to_vec());
        Ok(())
    }
}

#[test]
fn concurrent_first_runs_share_one_key() {
    let dir = tempfile::tempdir().unwrap();
    let path = Arc::new(dir.path().join("pin.db"));
    let keys = Arc::new(SlowKeys::default());
    let handles: Vec<_> = (0..2)
        .map(|_| {
            let (path, keys) = (Arc::clone(&path), Arc::clone(&keys));
            std::thread::spawn(move || Store::open_with(&path, &*keys, options()).map(drop))
        })
        .collect();
    for handle in handles {
        handle.join().unwrap().unwrap();
    }
    assert_eq!(*keys.saves.lock().unwrap(), 1);
}
