#![expect(clippy::unwrap_used)]

mod common;

use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use common::{MemoryKeys, key, options, raw};
use pin_core::{Error, Format, FormatKind, Key, KeyStore, NewEntry, Options, Pin, PinSlot, Store};

fn open(dir: &Path, history_limit: usize) -> Store {
    Store::open(
        dir.join("pin.db"),
        &key(),
        Options {
            history_limit: NonZeroUsize::new(history_limit).unwrap(),
            ..Options::default()
        },
    )
    .unwrap()
}

fn text(s: &str) -> NewEntry {
    NewEntry {
        formats: vec![Format::new(FormatKind::Text, s)],
        source_app: None,
    }
}

fn previews(entries: &[pin_core::Entry]) -> Vec<&str> {
    entries.iter().map(|e| e.preview.as_str()).collect()
}

#[test]
fn insert_roundtrips_all_formats() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path(), 100);
    let formats = vec![
        Format::new(FormatKind::Text, "hello"),
        Format::new(FormatKind::Html, "<b>hello</b>"),
        Format::new(FormatKind::Other("com.example.custom".into()), [1u8, 2, 3]),
    ];
    let id = store
        .insert(NewEntry {
            formats: formats.clone(),
            source_app: Some("Notes".into()),
        })
        .unwrap();

    let entry = store.get(id).unwrap();
    assert_eq!(entry.kind, FormatKind::Text);
    assert_eq!(entry.preview, "hello");
    assert_eq!(entry.source_app.as_deref(), Some("Notes"));
    assert_eq!(entry.size, 5 + 12 + 3);

    let mut stored = store.formats(id).unwrap();
    let mut expected = formats;
    stored.sort_by(|a, b| a.kind.cmp(&b.kind));
    expected.sort_by(|a, b| a.kind.cmp(&b.kind));
    assert_eq!(stored, expected);
}

#[test]
fn duplicate_copy_bumps_existing_entry() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path(), 100);
    let first = store.insert(text("one")).unwrap();
    store.insert(text("two")).unwrap();
    let again = store.insert(text("one")).unwrap();

    assert_eq!(first, again);
    assert_eq!(previews(&store.recent(10, 0).unwrap()), ["one", "two"]);
}

#[test]
fn touch_moves_entry_to_top() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path(), 100);
    let a = store.insert(text("a")).unwrap();
    store.insert(text("b")).unwrap();
    store.touch(a).unwrap();
    assert_eq!(previews(&store.recent(10, 0).unwrap()), ["a", "b"]);
}

#[test]
fn history_limit_prunes_oldest_unpinned_only() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path(), 2);
    let pinned = store.insert(text("pinned")).unwrap();
    store.set_pinned(pinned, true).unwrap();
    for s in ["1", "2", "3"] {
        store.insert(text(s)).unwrap();
    }
    assert_eq!(
        previews(&store.recent(10, 0).unwrap()),
        ["pinned", "3", "2"]
    );

    store.set_pinned(pinned, false).unwrap();
    assert_eq!(
        previews(&store.recent(10, 0).unwrap()),
        ["3", "2", "pinned"]
    );

    store.insert(text("4")).unwrap();
    assert_eq!(previews(&store.recent(10, 0).unwrap()), ["4", "3"]);
}

#[test]
fn pinned_entries_come_first() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path(), 100);
    let old = store.insert(text("old")).unwrap();
    store.insert(text("new")).unwrap();
    store.set_pinned(old, true).unwrap();
    let entries = store.recent(10, 0).unwrap();
    assert_eq!(previews(&entries), ["old", "new"]);
    assert!(entries[0].pin.is_pinned());
}

#[test]
fn slot_assignment_moves_slot_and_pins() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path(), 100);
    let a = store.insert(text("a")).unwrap();
    let b = store.insert(text("b")).unwrap();
    let slot = PinSlot::new(1).unwrap();

    store.set_slot(a, Some(slot)).unwrap();
    assert_eq!(store.get(a).unwrap().pin, Pin::Slot(slot));
    store.set_slot(b, Some(slot)).unwrap();
    assert_eq!(store.by_slot(slot).unwrap().map(|e| e.id), Some(b));
    assert_eq!(store.get(a).unwrap().pin, Pin::Pinned);

    store.set_pinned(b, false).unwrap();
    assert_eq!(store.by_slot(slot).unwrap(), None);
}

#[test]
fn invalid_slot_is_rejected() {
    assert!(matches!(PinSlot::new(0), Err(Error::InvalidSlot(0))));
    assert!(matches!(PinSlot::new(10), Err(Error::InvalidSlot(10))));
}

#[test]
fn long_query_matches_substrings_case_insensitively() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path(), 100);
    store.insert(text("Hello World")).unwrap();
    store.insert(text("another entry")).unwrap();

    assert_eq!(previews(&store.search("wor", 10).unwrap()), ["Hello World"]);
    assert_eq!(
        previews(&store.search("O WOR", 10).unwrap()),
        ["Hello World"]
    );
    assert!(store.search("missing", 10).unwrap().is_empty());
}

#[test]
fn short_query_scans_previews_literally() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path(), 100);
    store.insert(text("Hello World")).unwrap();
    store.insert(text("100% done")).unwrap();
    store.insert(text("a_b")).unwrap();

    assert_eq!(previews(&store.search("lo", 10).unwrap()), ["Hello World"]);
    assert_eq!(previews(&store.search("0%", 10).unwrap()), ["100% done"]);
    assert_eq!(previews(&store.search("_", 10).unwrap()), ["a_b"]);
}

#[test]
fn quotes_in_query_are_escaped() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path(), 100);
    store.insert(text("say \"quoted\" text")).unwrap();
    store.insert(text("plain text")).unwrap();

    assert_eq!(
        previews(&store.search("\"quoted\"", 10).unwrap()),
        ["say \"quoted\" text"]
    );
}

#[test]
fn blank_query_lists_recent_entries() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path(), 100);
    for s in ["a", "b", "c"] {
        store.insert(text(s)).unwrap();
    }
    assert_eq!(previews(&store.search("  ", 10).unwrap()), ["c", "b", "a"]);
    assert_eq!(store.search("", 2).unwrap().len(), 2);
}

#[test]
fn deleted_and_cleared_entries_leave_search() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path(), 100);
    let a = store.insert(text("alpha")).unwrap();
    let keep = store.insert(text("alpine")).unwrap();
    store.insert(text("alps")).unwrap();
    store.set_pinned(keep, true).unwrap();

    store.delete(a).unwrap();
    assert!(matches!(store.delete(a), Err(Error::NotFound(_))));
    assert_eq!(
        previews(&store.search("alp", 10).unwrap()),
        ["alpine", "alps"]
    );

    assert_eq!(store.clear_history().unwrap(), 1);
    assert_eq!(previews(&store.search("alp", 10).unwrap()), ["alpine"]);
}

#[test]
fn image_and_file_list_entries_get_kind_and_preview() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path(), 100);
    let image = store
        .insert(NewEntry {
            formats: vec![Format::new(FormatKind::Png, [0x89u8, b'P', b'N', b'G'])],
            source_app: None,
        })
        .unwrap();
    let files = store
        .insert(NewEntry {
            formats: vec![
                Format::new(FormatKind::Files, "file:///Users/me/report.pdf\n"),
                Format::new(FormatKind::Text, "report.pdf"),
            ],
            source_app: None,
        })
        .unwrap();

    assert_eq!(store.get(image).unwrap().kind, FormatKind::Png);
    assert_eq!(store.get(image).unwrap().preview, "Image");
    assert_eq!(store.get(files).unwrap().kind, FormatKind::Files);
    assert_eq!(store.get(files).unwrap().preview, "report.pdf");
    assert_eq!(
        previews(&store.search("report", 10).unwrap()),
        ["report.pdf"]
    );
}

#[test]
fn empty_entry_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path(), 100);
    assert!(matches!(
        store.insert(NewEntry::default()),
        Err(Error::Empty)
    ));
}

#[test]
fn reopen_requires_same_key() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pin.db");
    {
        let mut store = Store::open(&path, &key(), Options::default()).unwrap();
        store.insert(text("persisted")).unwrap();
    }
    let wrong = Store::open(
        &path,
        &Key::from_bytes(&mut [8; Key::LEN]),
        Options::default(),
    );
    assert!(matches!(wrong, Err(Error::WrongKey)));

    let store = Store::open(&path, &key(), Options::default()).unwrap();
    assert_eq!(previews(&store.recent(10, 0).unwrap()), ["persisted"]);
}

fn assert_no_plaintext(dir: &Path, marker: &str) -> usize {
    let mut files = 0;
    for file in std::fs::read_dir(dir).unwrap() {
        let bytes = std::fs::read(file.unwrap().path()).unwrap();
        assert!(!bytes.windows(marker.len()).any(|w| w == marker.as_bytes()));
        assert!(!bytes.starts_with(b"SQLite format 3"));
        files += 1;
    }
    files
}

#[test]
fn nothing_is_stored_in_plaintext() {
    let dir = tempfile::tempdir().unwrap();
    let marker = "plaintext-marker-4f1c";
    {
        let mut store = open(dir.path(), 100);
        store.insert(text(marker)).unwrap();
        assert!(dir.path().join("pin.db-wal").exists());
        assert!(assert_no_plaintext(dir.path(), marker) >= 2);
    }
    assert_no_plaintext(dir.path(), marker);
}

#[test]
fn recopy_of_pruned_entry_is_inserted_again() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path(), 1);
    let first = store.insert(text("a")).unwrap();
    store.insert(text("b")).unwrap();
    let again = store.insert(text("a")).unwrap();
    assert_ne!(first, again);
    assert_eq!(previews(&store.recent(10, 0).unwrap()), ["a"]);
}

#[test]
fn set_slot_on_missing_entry_keeps_existing_slot() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path(), 100);
    let a = store.insert(text("a")).unwrap();
    let b = store.insert(text("b")).unwrap();
    let slot = PinSlot::new(3).unwrap();
    store.set_slot(a, Some(slot)).unwrap();
    store.delete(b).unwrap();

    assert!(matches!(
        store.set_slot(b, Some(slot)),
        Err(Error::NotFound(_))
    ));
    assert_eq!(store.by_slot(slot).unwrap().map(|e| e.id), Some(a));
}

#[test]
fn short_queries_fold_unicode_case() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path(), 100);
    store.insert(text("Привет мир")).unwrap();
    assert_eq!(previews(&store.search("пр", 10).unwrap()), ["Привет мир"]);
    assert_eq!(previews(&store.search("ПРИ", 10).unwrap()), ["Привет мир"]);
}

#[test]
fn unencrypted_file_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("plain.db");
    let mut page = b"SQLite format 3\0".to_vec();
    page.resize(4096, 0);
    std::fs::write(&path, page).unwrap();
    let opened = Store::open(&path, &key(), Options::default());
    assert!(matches!(opened, Err(Error::WrongKey)));
}

#[test]
fn use_seq_lookup_is_served_by_index() {
    let dir = tempfile::tempdir().unwrap();
    drop(open(dir.path(), 100));
    let plan: Vec<String> = raw(dir.path())
        .prepare("EXPLAIN QUERY PLAN SELECT COALESCE(MAX(use_seq), 0) + 1 FROM entries")
        .unwrap()
        .query_map([], |row| row.get(3))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert!(
        plan.iter().any(|step| step.contains("entries_seq")),
        "{plan:?}"
    );
}

#[test]
fn negative_stored_size_is_a_conversion_error() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path(), 100);
    let id = store.insert(text("a")).unwrap();
    raw(dir.path())
        .execute("UPDATE entries SET size = -1", [])
        .unwrap();
    assert!(matches!(store.get(id), Err(Error::Sqlite(_))));
}

#[test]
fn key_source_is_zeroized() {
    let mut bytes = [9; Key::LEN];
    let key = Key::from_bytes(&mut bytes);
    assert_eq!(bytes, [0; Key::LEN]);
    assert_eq!(key.as_bytes(), &[9; Key::LEN]);
}

#[test]
fn open_with_keys_creates_key_once_and_refuses_lost_key() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pin.db");
    let keys = MemoryKeys::default();
    let mut store = Store::open_with_keys(&path, &keys, options()).unwrap();
    store.insert(text("hello")).unwrap();
    drop(store);

    let store = Store::open_with_keys(&path, &keys, options()).unwrap();
    assert_eq!(store.recent(10, 0).unwrap().len(), 1);
    drop(store);

    assert!(matches!(
        Store::open_with_keys(&path, &MemoryKeys::default(), options()),
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
        stored.as_deref().map(Key::try_from).transpose()
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
            std::thread::spawn(move || Store::open_with_keys(&*path, &*keys, options()).map(drop))
        })
        .collect();
    for handle in handles {
        handle.join().unwrap().unwrap();
    }
    assert_eq!(*keys.saves.lock().unwrap(), 1);
}
