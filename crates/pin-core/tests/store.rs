#![allow(clippy::unwrap_used)]

use std::path::Path;

use pin_core::{Error, Format, FormatKind, Key, NewEntry, Options, PinSlot, Store};

fn key() -> Key {
    Key::from_bytes(&mut [7; Key::LEN])
}

fn open(dir: &Path, history_limit: usize) -> Store {
    Store::open(
        &dir.join("pin.db"),
        &key(),
        Options {
            history_limit,
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
    assert!(entries[0].pinned);
}

#[test]
fn slot_assignment_moves_slot_and_pins() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path(), 100);
    let a = store.insert(text("a")).unwrap();
    let b = store.insert(text("b")).unwrap();
    let slot = PinSlot::new(1).unwrap();

    store.set_slot(a, Some(slot)).unwrap();
    assert!(store.get(a).unwrap().pinned);
    store.set_slot(b, Some(slot)).unwrap();
    assert_eq!(store.by_slot(slot).unwrap().map(|e| e.id), Some(b));
    assert_eq!(store.get(a).unwrap().slot, None);
    assert!(store.get(a).unwrap().pinned);

    store.set_pinned(b, false).unwrap();
    assert_eq!(store.by_slot(slot).unwrap(), None);
}

#[test]
fn invalid_slot_is_rejected() {
    assert!(matches!(PinSlot::new(0), Err(Error::InvalidSlot(0))));
    assert!(matches!(PinSlot::new(10), Err(Error::InvalidSlot(10))));
}

#[test]
fn search_finds_substrings_case_insensitive() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path(), 100);
    store.insert(text("Hello World")).unwrap();
    store.insert(text("100% done")).unwrap();
    store.insert(text("say \"quoted\" text")).unwrap();
    store.insert(text("a_b")).unwrap();

    assert_eq!(previews(&store.search("wor", 10).unwrap()), ["Hello World"]);
    assert_eq!(previews(&store.search("lo", 10).unwrap()), ["Hello World"]);
    assert_eq!(previews(&store.search("0%", 10).unwrap()), ["100% done"]);
    assert_eq!(previews(&store.search("_", 10).unwrap()), ["a_b"]);
    assert_eq!(
        previews(&store.search("\"quoted\"", 10).unwrap()),
        ["say \"quoted\" text"]
    );
    assert!(store.search("missing", 10).unwrap().is_empty());
    assert_eq!(store.search("  ", 10).unwrap().len(), 4);
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
fn image_and_files_entries() {
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
fn zero_history_limit_keeps_latest_entry() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = open(dir.path(), 0);
    store.insert(text("a")).unwrap();
    let b = store.insert(text("b")).unwrap();
    assert_eq!(store.get(b).unwrap().preview, "b");
    assert_eq!(previews(&store.recent(10, 0).unwrap()), ["b"]);
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
fn key_source_is_zeroized() {
    let mut bytes = [9; Key::LEN];
    let key = Key::from_bytes(&mut bytes);
    assert_eq!(bytes, [0; Key::LEN]);
    assert_eq!(key.as_bytes(), &[9; Key::LEN]);
}
