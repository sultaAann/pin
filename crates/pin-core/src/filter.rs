use crate::model::{Entry, FormatKind};

/// Which part of the history a list or search covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    #[default]
    All,
    Text,
    Links,
    Images,
    Files,
    Pinned,
}

impl Filter {
    pub const ALL: [Self; 6] = [
        Self::All,
        Self::Text,
        Self::Links,
        Self::Images,
        Self::Files,
        Self::Pinned,
    ];

    pub fn matches(self, entry: &Entry) -> bool {
        match self {
            Self::All => true,
            Self::Text => matches!(
                entry.kind,
                FormatKind::Text | FormatKind::Html | FormatKind::Rtf
            ),
            Self::Links => entry.kind == FormatKind::Text && is_link(&entry.preview),
            Self::Images => entry.kind == FormatKind::Png,
            Self::Files => entry.kind == FormatKind::Files,
            Self::Pinned => entry.pin.is_pinned(),
        }
    }

    /// SQL condition on `entries e` that selects exactly the entries `matches` accepts.
    pub(crate) const fn sql(self) -> &'static str {
        match self {
            Self::All => "1",
            Self::Text => "e.kind IN ('text/plain', 'text/html', 'text/rtf')",
            Self::Links => {
                "e.kind = 'text/plain' AND instr(e.preview, ' ') = 0 \
                 AND (e.preview LIKE 'http://%' OR e.preview LIKE 'https://%')"
            }
            Self::Images => "e.kind = 'image/png'",
            Self::Files => "e.kind = 'text/uri-list'",
            Self::Pinned => "(e.pinned = 1 OR e.pin_slot IS NOT NULL)",
        }
    }
}

// Previews collapse whitespace to single spaces, so a space means more than a bare URL; LIKE in
// `sql` folds ASCII case, and so does this.
fn is_link(preview: &str) -> bool {
    let scheme = |prefix: &str| {
        preview
            .get(..prefix.len())
            .is_some_and(|head| head.eq_ignore_ascii_case(prefix))
    };
    !preview.contains(' ') && (scheme("http://") || scheme("https://"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_bare_urls_are_links() {
        assert!(is_link("https://example.com/a"));
        assert!(is_link("HTTP://EXAMPLE.COM"));
        assert!(!is_link("see https://example.com"));
        assert!(!is_link("example.com"));
        assert!(!is_link("https"));
        assert!(!is_link("ftp://example.com"));
    }
}
