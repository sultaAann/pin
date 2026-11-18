use pin_core::{Entry, FormatKind, Timestamp};

pub fn title(entry: &Entry) -> &str {
    entry
        .preview
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_else(|| entry.kind.as_str())
}

pub fn icon(entry: &Entry) -> &'static str {
    match entry.kind {
        FormatKind::Png => "\u{1F5BC}",
        FormatKind::Files => "\u{1F4C1}",
        FormatKind::Html | FormatKind::Rtf => "\u{1F4DD}",
        FormatKind::Text if is_link(title(entry)) => "\u{1F517}",
        FormatKind::Text => "\u{1F4C4}",
        FormatKind::Other(_) => "\u{1F4CB}",
    }
}

fn is_link(text: &str) -> bool {
    !text.contains(char::is_whitespace)
        && (text.starts_with("https://") || text.starts_with("http://"))
}

pub fn details(entry: &Entry, now: Timestamp) -> String {
    let when = relative(now, entry.last_used_at);
    let size = size(entry.size);
    match &entry.source_app {
        Some(app) => format!("{app} \u{00B7} {when} \u{00B7} {size}"),
        None => format!("{when} \u{00B7} {size}"),
    }
}

fn relative(now: Timestamp, then: Timestamp) -> String {
    let seconds = now.as_millis().saturating_sub(then.as_millis()) / 1000;
    match seconds {
        ..60 => "Just now".to_owned(),
        60..3_600 => format!("{} min ago", seconds / 60),
        3_600..86_400 => format!("{} h ago", seconds / 3_600),
        _ => format!("{} d ago", seconds / 86_400),
    }
}

fn size(bytes: u64) -> String {
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
    fn only_bare_urls_count_as_links() {
        assert!(is_link("https://example.com/a"));
        assert!(!is_link("see https://example.com"));
        assert!(!is_link("example.com"));
    }
}
