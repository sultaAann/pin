use std::borrow::Cow;

use zeroize::Zeroize;

use crate::model::{Format, FormatKind};

const PREVIEW_CHARS: usize = 200;
pub(crate) const MAX_INDEXED_BYTES: usize = 64 * 1024;

pub(crate) fn normalize(mut formats: Vec<Format>) -> Vec<Format> {
    formats.sort_by(|a, b| a.kind.cmp(&b.kind));
    formats.dedup_by(|dropped, kept| {
        let duplicate = dropped.kind == kept.kind;
        if duplicate {
            dropped.data.zeroize();
        }
        duplicate
    });
    formats
}

pub(crate) fn hash(formats: &[Format]) -> blake3::Hash {
    let mut hasher = blake3::Hasher::new();
    for format in formats {
        let kind = format.kind.as_str().as_bytes();
        hasher.update(&len_u64(kind.len()).to_le_bytes());
        hasher.update(kind);
        hasher.update(&len_u64(format.data.len()).to_le_bytes());
        hasher.update(&format.data);
    }
    hasher.finalize()
}

fn len_u64(len: usize) -> u64 {
    u64::try_from(len).unwrap_or(u64::MAX)
}

pub(crate) fn primary_kind(formats: &[Format]) -> FormatKind {
    const PRIORITY: [FormatKind; 5] = [
        FormatKind::Png,
        FormatKind::Files,
        FormatKind::Text,
        FormatKind::Html,
        FormatKind::Rtf,
    ];
    PRIORITY
        .into_iter()
        .find(|kind| formats.iter().any(|f| &f.kind == kind))
        .or_else(|| formats.first().map(|f| f.kind.clone()))
        .unwrap_or(FormatKind::Text)
}

pub(crate) fn search_text(formats: &[Format]) -> Option<Cow<'_, str>> {
    let text = match find(formats, &FormatKind::Text) {
        Some(text) => String::from_utf8_lossy(truncate_bytes(text, MAX_INDEXED_BYTES)),
        None => Cow::Owned(files_text(find(formats, &FormatKind::Files)?)),
    };
    (!text.trim().is_empty()).then_some(text)
}

fn files_text(data: &[u8]) -> String {
    file_paths(data).join("\n")
}

/// Decoded paths of a `text/uri-list`, reading at most the first 64 KiB; a path cut by that
/// limit is dropped rather than shown as a different file.
pub(crate) fn file_paths(data: &[u8]) -> Vec<String> {
    let cut = data.len() > MAX_INDEXED_BYTES;
    let list = String::from_utf8_lossy(truncate_bytes(data, MAX_INDEXED_BYTES));
    let list = match list.rfind('\n') {
        Some(end) if cut => &list[..end],
        _ if cut => "",
        _ => &list,
    };
    file_uris(list)
        .map(|uri| percent_decode(uri.strip_prefix("file://").unwrap_or(uri)))
        .collect()
}

/// Text cut to at most `max` bytes on a char boundary.
pub(crate) fn display_text(data: &[u8], max: usize) -> String {
    String::from_utf8_lossy(truncate_bytes(data, max)).into_owned()
}

fn into_string_lossy(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

pub(crate) fn preview(formats: &[Format], kind: &FormatKind) -> String {
    match kind {
        FormatKind::Png => {
            let name =
                find(formats, &FormatKind::Files).map_or_else(|| "Image".to_owned(), files_preview);
            match find(formats, kind).and_then(crate::thumbnail::png_size) {
                Some((w, h)) => format!("{name} \u{00B7} {w}\u{00D7}{h}"),
                None => name,
            }
        }
        FormatKind::Files => files_preview(find(formats, kind).unwrap_or_default()),
        FormatKind::Text | FormatKind::Html | FormatKind::Rtf | FormatKind::Other(_) => {
            find(formats, &FormatKind::Text)
                .map(text_preview)
                .filter(|p| !p.is_empty())
                .unwrap_or_else(|| kind.as_str().to_owned())
        }
    }
}

pub(crate) fn size(formats: &[Format]) -> usize {
    formats.iter().map(|f| f.data.len()).sum()
}

fn find<'a>(formats: &'a [Format], kind: &FormatKind) -> Option<&'a [u8]> {
    formats
        .iter()
        .find(|f| &f.kind == kind)
        .map(|f| f.data.as_slice())
}

fn truncate_bytes(data: &[u8], max: usize) -> &[u8] {
    if data.len() <= max {
        return data;
    }
    let mut end = max;
    while end > 0 && (data[end] & 0b1100_0000) == 0b1000_0000 {
        end -= 1;
    }
    &data[..end]
}

fn text_preview(data: &[u8]) -> String {
    let start = data
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(data.len());
    let text = String::from_utf8_lossy(truncate_bytes(&data[start..], PREVIEW_CHARS * 4));
    text.split_whitespace()
        .flat_map(|word| std::iter::once(' ').chain(word.chars()))
        .skip(1)
        .take(PREVIEW_CHARS)
        .collect()
}

fn files_preview(data: &[u8]) -> String {
    let list = String::from_utf8_lossy(data);
    let mut files = file_uris(&list);
    let Some(first) = files.next() else {
        return "Files".to_owned();
    };
    let name = first
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(first);
    let name = percent_decode(name);
    match files.count() {
        0 => name,
        rest => format!("{name} (+{rest})"),
    }
}

fn file_uris(list: &str) -> impl Iterator<Item = &str> {
    list.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
}

fn percent_decode(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    percent_decode_into(&mut out, s);
    into_string_lossy(out)
}

fn percent_decode_into(out: &mut Vec<u8>, s: &str) {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|h| std::str::from_utf8(h).ok())
            .and_then(|h| u8::from_str_radix(h, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (byte, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn hash_ignores_format_order_and_duplicates() {
        let a = normalize(vec![
            Format::new(FormatKind::Text, "hi"),
            Format::new(FormatKind::Html, "<b>hi</b>"),
        ]);
        let b = normalize(vec![
            Format::new(FormatKind::Html, "<b>hi</b>"),
            Format::new(FormatKind::Text, "hi"),
            Format::new(FormatKind::Text, "other"),
        ]);
        assert_eq!(hash(&a), hash(&b));
    }

    #[test]
    fn text_preview_collapses_whitespace_and_truncates() {
        let long = format!("  a \n\t b  {}", "x".repeat(500));
        let preview = text_preview(long.as_bytes());
        assert!(preview.starts_with("a b x"));
        assert_eq!(preview.chars().count(), PREVIEW_CHARS);
    }

    #[test]
    fn truncate_keeps_utf8_boundary() {
        let text = "ééé";
        assert_eq!(truncate_bytes(text.as_bytes(), 3), "é".as_bytes());
    }

    #[test]
    fn files_preview_shows_first_name_and_count() {
        let list = b"file:///tmp/a.txt\r\nfile:///tmp/dir/\r\nfile:///tmp/c.png\r\n";
        assert_eq!(files_preview(list), "a.txt (+2)");
    }

    #[test]
    fn files_are_indexed_and_previewed_decoded() {
        let formats = [Format::new(
            FormatKind::Files,
            "file:///Users/me/my%20report%D1%8B.pdf\r\n",
        )];
        assert_eq!(
            search_text(&formats).as_deref(),
            Some("/Users/me/my reportы.pdf")
        );
        assert_eq!(files_preview(&formats[0].data), "my reportы.pdf");
        assert_eq!(percent_decode("100%"), "100%");
    }

    #[test]
    fn file_list_cut_by_limit_drops_partial_path() {
        let line = "file:///tmp/a.txt\n";
        let full = line.repeat(MAX_INDEXED_BYTES / line.len());
        let long = format!("{full}file:///tmp/partial-name-that-goes-past-the-limit.txt\n");
        let paths = file_paths(long.as_bytes());
        assert_eq!(paths.len(), MAX_INDEXED_BYTES / line.len());
        assert!(paths.iter().all(|p| p == "/tmp/a.txt"));
        assert_eq!(file_paths(b"file:///tmp/b.txt"), ["/tmp/b.txt"]);
    }

    #[test]
    fn indexed_text_is_cut_on_char_boundary() {
        let text = "я".repeat(MAX_INDEXED_BYTES);
        let formats = [Format::new(FormatKind::Text, text)];
        let indexed = search_text(&formats).unwrap();
        assert!(indexed.len() <= MAX_INDEXED_BYTES);
        assert!(indexed.chars().all(|c| c == 'я'));
    }

    #[test]
    fn preview_skips_leading_whitespace() {
        let text = format!("{}hello", " ".repeat(2000));
        assert_eq!(text_preview(text.as_bytes()), "hello");
    }

    #[test]
    fn primary_kind_prefers_files_then_image_then_text() {
        let formats = [
            Format::new(FormatKind::Text, "a"),
            Format::new(FormatKind::Png, [0u8]),
        ];
        assert_eq!(primary_kind(&formats), FormatKind::Png);
    }

    proptest! {
        #[test]
        fn truncate_bytes_keeps_valid_utf8_within_limit(s in ".*", max in 0usize..64) {
            let cut = truncate_bytes(s.as_bytes(), max);
            prop_assert!(cut.len() <= max);
            prop_assert!(std::str::from_utf8(cut).is_ok());
        }

        #[test]
        fn text_preview_is_bounded_and_trimmed(data in proptest::collection::vec(any::<u8>(), 0..2048)) {
            let preview = text_preview(&data);
            prop_assert!(preview.chars().count() <= PREVIEW_CHARS);
            prop_assert!(!preview.starts_with(char::is_whitespace));
        }

        #[test]
        fn percent_decode_never_panics(s in ".*") {
            let _ = percent_decode(&s);
        }

        #[test]
        fn files_text_never_panics(data in proptest::collection::vec(any::<u8>(), 0..512)) {
            let _ = files_text(&data);
        }
    }
}
