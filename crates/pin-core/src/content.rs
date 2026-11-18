use zeroize::Zeroize;

use crate::model::{Format, FormatKind};

const PREVIEW_CHARS: usize = 200;
const MAX_INDEXED_BYTES: usize = 64 * 1024;

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
        hasher.update(&(kind.len() as u64).to_le_bytes());
        hasher.update(kind);
        hasher.update(&(format.data.len() as u64).to_le_bytes());
        hasher.update(&format.data);
    }
    hasher.finalize()
}

pub(crate) fn primary_kind(formats: &[Format]) -> FormatKind {
    const PRIORITY: [FormatKind; 5] = [
        FormatKind::Files,
        FormatKind::Png,
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

pub(crate) fn search_text(formats: &[Format]) -> Option<String> {
    let text = if let Some(text) = find(formats, &FormatKind::Text) {
        String::from_utf8_lossy(truncate_bytes(text, MAX_INDEXED_BYTES)).into_owned()
    } else {
        files_text(find(formats, &FormatKind::Files)?)
    };
    (!text.trim().is_empty()).then_some(text)
}

fn files_text(data: &[u8]) -> String {
    let list = String::from_utf8_lossy(data);
    file_uris(&list)
        .map(|uri| percent_decode(uri.strip_prefix("file://").unwrap_or(uri)))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn preview(formats: &[Format], kind: &FormatKind) -> String {
    match kind {
        FormatKind::Png => "Image".to_owned(),
        FormatKind::Files => files_preview(find(formats, kind).unwrap_or_default()),
        _ => find(formats, &FormatKind::Text)
            .map(text_preview)
            .filter(|p| !p.is_empty())
            .unwrap_or_else(|| kind.as_str().to_owned()),
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
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
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
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
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
}
