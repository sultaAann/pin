use super::icons::Icon;

const TABLE: [(Icon, &[&str]); 8] = [
    (
        Icon::Doc,
        &["txt", "md", "doc", "docx", "pages", "rtf", "odt"],
    ),
    (Icon::Pdf, &["pdf"]),
    (
        Icon::Code,
        &[
            "rs", "js", "jsx", "ts", "tsx", "py", "go", "rb", "java", "kt", "swift", "c", "h",
            "cpp", "cs", "php", "sh", "zsh", "json", "yml", "yaml", "toml", "xml", "html", "css",
            "scss", "sql", "lock", "env", "ini", "conf",
        ],
    ),
    (
        Icon::Image,
        &[
            "png", "jpg", "jpeg", "heic", "heif", "gif", "svg", "webp", "avif", "bmp", "tif",
            "tiff", "fig", "psd", "sketch",
        ],
    ),
    (
        Icon::Media,
        &[
            "mp4", "mov", "mkv", "avi", "webm", "mp3", "wav", "m4a", "aac", "flac", "ogg",
        ],
    ),
    (
        Icon::Archive,
        &[
            "zip", "tar", "gz", "tgz", "bz2", "xz", "7z", "rar", "dmg", "iso", "pkg",
        ],
    ),
    (
        Icon::Sheet,
        &["csv", "tsv", "xls", "xlsx", "numbers", "ods"],
    ),
    (Icon::App, &["app", "exe", "msi", "appimage"]),
];

// Build and tool files usually have no extension but are code all the same.
const CODE_NAMES: [&str; 7] = [
    "makefile",
    "dockerfile",
    "gemfile",
    "justfile",
    "rakefile",
    "procfile",
    "brewfile",
];

/// Icon for a file `name`, picked by extension (case-insensitive); a trailing slash means a folder.
pub fn icon(name: &str) -> Icon {
    if let Some(folder) = name.strip_suffix('/') {
        // App bundles are folders on disk.
        let is_app = folder.to_ascii_lowercase().ends_with(".app");
        return if is_app { Icon::App } else { Icon::Folder };
    }
    let name = name.rsplit('/').next().unwrap_or(name).to_ascii_lowercase();
    if CODE_NAMES.contains(&name.as_str()) {
        return Icon::Code;
    }
    let Some((stem, extension)) = name.rsplit_once('.') else {
        return Icon::File;
    };
    if stem.is_empty() {
        // Dotfiles such as .gitignore or .zshrc are configuration.
        return Icon::Code;
    }
    TABLE
        .iter()
        .find(|(_, extensions)| extensions.contains(&extension))
        .map_or(Icon::File, |(icon, _)| *icon)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_pick_a_family() {
        assert_eq!(icon("global.json"), Icon::Code);
        assert_eq!(icon("docker-compose copy.yml"), Icon::Code);
        assert_eq!(icon("Makefile"), Icon::Code);
        assert_eq!(icon(".gitignore"), Icon::Code);
        assert_eq!(icon("Q3 report.PDF"), Icon::Pdf);
        assert_eq!(icon("notes.md"), Icon::Doc);
        assert_eq!(icon("photo.HEIC"), Icon::Image);
        assert_eq!(icon("clip.mov"), Icon::Media);
        assert_eq!(icon("release-0.4.dmg"), Icon::Archive);
        assert_eq!(icon("budget.numbers"), Icon::Sheet);
        assert_eq!(icon("Pin.app/"), Icon::App);
        assert_eq!(icon("Pin.app"), Icon::App);
        assert_eq!(icon("/Users/me/libs/"), Icon::Folder);
        assert_eq!(icon("README"), Icon::File);
        assert_eq!(icon("archive.unknown"), Icon::File);
    }
}
