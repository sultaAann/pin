use pin_core::{Entry, Filter, FormatKind, Timestamp};

use super::icons::Icon;

pub fn title(entry: &Entry) -> &str {
    entry
        .preview
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_else(|| entry.kind.as_str())
}

pub fn icon(entry: &Entry) -> Icon {
    match entry.kind {
        FormatKind::Png => Icon::Image,
        FormatKind::Files => Icon::Folder,
        FormatKind::Html | FormatKind::Rtf => Icon::Rich,
        FormatKind::Text if Filter::Links.matches(entry) => Icon::Link,
        FormatKind::Text => Icon::Text,
        FormatKind::Other(_) => Icon::Clipboard,
    }
}

pub fn subtitle(entry: &Entry, now: Timestamp) -> String {
    let when = relative(now, entry.last_used_at);
    match &entry.source_app {
        Some(app) => format!("{app} \u{00B7} {when}"),
        None => when,
    }
}

pub fn kind_label(entry: &Entry, files: usize) -> String {
    match entry.kind {
        FormatKind::Png => "Image".to_owned(),
        FormatKind::Files if files == 1 => "1 File".to_owned(),
        FormatKind::Files => format!("{files} Files"),
        FormatKind::Html | FormatKind::Rtf => "Rich Text".to_owned(),
        FormatKind::Text if Filter::Links.matches(entry) => "Link".to_owned(),
        FormatKind::Text => "Plain Text".to_owned(),
        FormatKind::Other(_) => "Clipboard Data".to_owned(),
    }
}

pub fn format_name(kind: &FormatKind) -> &str {
    match kind {
        FormatKind::Text => "Plain Text",
        FormatKind::Html => "HTML",
        FormatKind::Rtf => "RTF",
        FormatKind::Png => "PNG",
        FormatKind::Files => "File URLs",
        FormatKind::Other(mime) => mime,
    }
}

pub fn filter_label(filter: Filter) -> &'static str {
    match filter {
        Filter::All => "All",
        Filter::Text => "Text",
        Filter::Links => "Links",
        Filter::Images => "Images",
        Filter::Files => "Files",
        Filter::Pinned => "Pinned",
    }
}

/// Plural noun for the empty state of `filter`, e.g. "No images in your clipboard history".
pub fn filter_noun(filter: Filter) -> &'static str {
    match filter {
        Filter::All => "items",
        Filter::Text => "text",
        Filter::Links => "links",
        Filter::Images => "images",
        Filter::Files => "files",
        Filter::Pinned => "pinned items",
    }
}

/// Host of a bare URL, for the link preview header.
pub fn domain(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.split(['/', '?', '#']).next().unwrap_or(rest)
}

pub fn relative(now: Timestamp, then: Timestamp) -> String {
    let seconds = now.as_millis().saturating_sub(then.as_millis()) / 1000;
    match seconds {
        ..60 => "Just now".to_owned(),
        60..3_600 => format!("{} min ago", seconds / 60),
        3_600..86_400 => format!("{} h ago", seconds / 3_600),
        _ => format!("{} d ago", seconds / 86_400),
    }
}

pub fn size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    match bytes {
        ..KB => format!("{bytes} B"),
        KB..MB => format!("{} KB", bytes.div_ceil(KB)),
        _ => format!("{}.{} MB", bytes / MB, bytes % MB * 10 / MB),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_time_uses_coarse_units() {
        let now = Timestamp::from_millis(10_000_000);
        let ago = |s: i64| relative(now, Timestamp::from_millis(10_000_000 - s * 1000));
        assert_eq!(ago(5), "Just now");
        assert_eq!(ago(125), "2 min ago");
        assert_eq!(ago(7_200), "2 h ago");
        assert_eq!(ago(9_000), "2 h ago");
    }

    #[test]
    fn sizes_are_human_readable() {
        assert_eq!(size(12), "12 B");
        assert_eq!(size(1_500), "2 KB");
        assert_eq!(size(5 * 1024 * 1024 + 512 * 1024), "5.5 MB");
    }

    #[test]
    fn domain_is_the_host_of_a_url() {
        assert_eq!(
            domain("https://doc.rust-lang.org/std/"),
            "doc.rust-lang.org"
        );
        assert_eq!(domain("http://example.com?q=1"), "example.com");
        assert_eq!(domain("https://crates.io"), "crates.io");
    }
}
