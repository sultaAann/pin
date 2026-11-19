#![expect(clippy::unwrap_used)]

use std::hint::black_box;
use std::num::NonZeroUsize;

use criterion::{Criterion, criterion_group, criterion_main};
use pin_core::{Format, FormatKind, Key, NewEntry, Options, Store};

const ENTRIES: usize = 10_000;

fn seeded_store(dir: &std::path::Path) -> Store {
    let options = Options {
        history_limit: NonZeroUsize::new(ENTRIES * 2).unwrap(),
        ..Options::default()
    };
    let mut store = Store::open(
        dir.join("pin.db"),
        &Key::from_bytes(&mut [7; Key::LEN]),
        options,
    )
    .unwrap();
    for i in 0..ENTRIES {
        let text = format!(
            "entry {i} lorem ipsum dolor sit amet {} consectetur",
            i.wrapping_mul(7919) % 10_007
        );
        store
            .insert(NewEntry {
                formats: vec![Format::new(FormatKind::Text, text)],
                source_app: None,
            })
            .unwrap();
    }
    store
}

fn search(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    let store = seeded_store(dir.path());
    let mut group = c.benchmark_group("search");
    for query in ["e", "ps", "lore"] {
        group.bench_function(format!("{}_chars", query.chars().count()), |b| {
            b.iter(|| {
                store
                    .search(black_box(query), pin_core::Filter::All, 50)
                    .unwrap()
            });
        });
    }
    group.finish();
}

criterion_group!(benches, search);
criterion_main!(benches);
