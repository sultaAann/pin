use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep};
use objc2_foundation::{NSData, NSDictionary, NSString, NSURL};
use pin_core::{Format, FormatKind};

use super::*;
use crate::{Capture, Skip};

const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
const TINY_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0xf0,
    0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

fn entry(capture: Capture) -> Vec<Format> {
    match capture {
        Capture::Entry(entry) => entry.formats,
        other => panic!("expected an entry, got {other:?}"),
    }
}

fn kinds(formats: &[Format]) -> Vec<FormatKind> {
    formats.iter().map(|f| f.kind.clone()).collect()
}

/// A fresh directory with empty files `names`; links written back must point at real files.
fn temp_files(test: &str, names: &[&str]) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("pin-clip-{}-{test}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for name in names {
        std::fs::write(dir.join(name), b"x").unwrap();
    }
    dir
}

fn url(dir: &std::path::Path, name: &str) -> String {
    format!("file://{}/{}", dir.display(), name.replace(' ', "%20"))
}

#[test]
fn written_formats_read_back_as_one_entry() {
    let clipboard = Clipboard::unique();
    let formats = [
        Format::new(FormatKind::Text, "hello"),
        Format::new(FormatKind::Html, "<b>hello</b>"),
        Format::new(FormatKind::Png, TINY_PNG),
    ];
    let before = clipboard.change_count();
    let after = clipboard.write(&formats, Privacy::Normal).unwrap();
    assert_ne!(before, after);
    assert_eq!(entry(clipboard.read()), formats);
}

#[test]
fn files_roundtrip_as_uri_list() {
    let dir = temp_files("roundtrip", &["a b.txt", "c.txt"]);
    let clipboard = Clipboard::unique();
    let list = format!("{}\r\n{}\r\n", url(&dir, "a b.txt"), url(&dir, "c.txt"));
    clipboard
        .write(
            &[Format::new(FormatKind::Files, list.as_str())],
            Privacy::Normal,
        )
        .unwrap();
    let formats = entry(clipboard.read());
    assert_eq!(formats, [Format::new(FormatKind::Files, list.as_str())]);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn file_reference_urls_are_stored_as_paths() {
    let dir = temp_files("reference", &["report.txt"]);
    let path = dir.join("report.txt");
    let reference = NSURL::fileURLWithPath(&NSString::from_str(&path.to_string_lossy()))
        .fileReferenceURL()
        .unwrap()
        .absoluteString()
        .unwrap()
        .to_string();
    assert!(reference.starts_with("file:///.file/id="), "{reference}");
    let clipboard = Clipboard::unique();
    clipboard.board.clearContents();
    assert!(clipboard.board.setData_forType(
        Some(&NSData::with_bytes(reference.as_bytes())),
        &NSString::from_str(FILE_URL)
    ));
    let formats = entry(clipboard.read());
    let list = String::from_utf8(formats[0].data.clone()).unwrap();
    assert!(list.starts_with("file:///"), "{list}");
    assert!(!list.contains("/.file/id="), "{list}");
    assert!(list.ends_with("/report.txt\r\n"), "{list}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn links_to_missing_files_are_not_written_back() {
    let clipboard = Clipboard::unique();
    let gone = Format::new(FormatKind::Files, "file:///nonexistent/pin-gone.txt\r\n");
    assert!(matches!(
        clipboard.write(std::slice::from_ref(&gone), Privacy::Normal),
        Err(crate::Error::NothingToWrite)
    ));
    clipboard
        .write(
            &[Format::new(FormatKind::Text, "pin-gone.txt"), gone],
            Privacy::Normal,
        )
        .unwrap();
    assert_eq!(kinds(&entry(clipboard.read())), [FormatKind::Text]);
}

#[test]
fn jpeg_only_images_are_stored_as_png() {
    let rep = NSBitmapImageRep::imageRepWithData(&NSData::with_bytes(TINY_PNG)).unwrap();
    // SAFETY: an empty dictionary is a valid properties argument.
    let jpeg = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::JPEG, &NSDictionary::new())
    }
    .unwrap();
    let clipboard = Clipboard::unique();
    clipboard.board.clearContents();
    assert!(
        clipboard
            .board
            .setData_forType(Some(&jpeg), &NSString::from_str(JPEG))
    );
    let formats = entry(clipboard.read());
    assert_eq!(kinds(&formats), [FormatKind::Png]);
    assert!(formats[0].data.starts_with(PNG_SIGNATURE));
}

#[test]
fn concealed_writes_are_not_captured() {
    let clipboard = Clipboard::unique();
    clipboard
        .write(
            &[Format::new(FormatKind::Text, "hunter2")],
            Privacy::Concealed,
        )
        .unwrap();
    assert_eq!(clipboard.read(), Capture::Skipped(Skip::Private));
}

#[test]
fn transient_and_auto_generated_copies_are_skipped() {
    for marker in [TRANSIENT, AUTO_GENERATED] {
        let clipboard = Clipboard::unique();
        clipboard.board.clearContents();
        let text = NSString::from_str(TEXT);
        let flag = NSString::from_str(marker);
        assert!(
            clipboard
                .board
                .setData_forType(Some(&NSData::with_bytes(b"x")), &text)
        );
        assert!(clipboard.board.setData_forType(Some(&NSData::new()), &flag));
        assert_eq!(
            clipboard.read(),
            Capture::Skipped(Skip::Private),
            "{marker}"
        );
    }
}

#[test]
fn tiff_only_images_are_stored_as_png() {
    let rep = NSBitmapImageRep::imageRepWithData(&NSData::with_bytes(TINY_PNG)).unwrap();
    // SAFETY: an empty dictionary is a valid properties argument.
    let tiff = unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::TIFF, &NSDictionary::new())
    }
    .unwrap();
    let clipboard = Clipboard::unique();
    clipboard.board.clearContents();
    assert!(
        clipboard
            .board
            .setData_forType(Some(&tiff), &NSString::from_str(TIFF))
    );
    let formats = entry(clipboard.read());
    assert_eq!(kinds(&formats), [FormatKind::Png]);
    assert!(formats[0].data.starts_with(PNG_SIGNATURE));
}

#[test]
fn private_marker_on_any_item_skips_the_copy() {
    let dir = temp_files("private", &["a", "b"]);
    let clipboard = Clipboard::unique();
    let list = format!("{}\r\n{}\r\n", url(&dir, "a"), url(&dir, "b"));
    clipboard
        .write(
            &[
                Format::new(FormatKind::Text, "a"),
                Format::new(FormatKind::Files, list.as_str()),
            ],
            Privacy::Normal,
        )
        .unwrap();
    let items = clipboard.board.pasteboardItems().unwrap().to_vec();
    assert!(items[1].setData_forType(&NSData::new(), &NSString::from_str(CONCEALED)));
    assert_eq!(clipboard.read(), Capture::Skipped(Skip::Private));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn png_is_preferred_over_tiff() {
    let clipboard = Clipboard::unique();
    clipboard.board.clearContents();
    let png = NSData::with_bytes(TINY_PNG);
    assert!(
        clipboard
            .board
            .setData_forType(Some(&png), &NSString::from_str(PNG))
    );
    assert!(clipboard.board.setData_forType(
        Some(&NSData::with_bytes(b"not a tiff")),
        &NSString::from_str(TIFF)
    ));
    assert_eq!(
        entry(clipboard.read()),
        [Format::new(FormatKind::Png, TINY_PNG)]
    );
}

#[test]
fn non_file_uris_are_not_written_as_files() {
    let clipboard = Clipboard::unique();
    let only_web = Format::new(FormatKind::Files, "https://example.com\r\n");
    assert!(matches!(
        clipboard.write(&[only_web], Privacy::Normal),
        Err(crate::Error::NothingToWrite)
    ));
}

#[test]
fn copies_over_the_limit_are_skipped() {
    let mut clipboard = Clipboard::unique();
    clipboard.limit = 8;
    clipboard
        .write(
            &[Format::new(FormatKind::Text, "123456789")],
            Privacy::Normal,
        )
        .unwrap();
    assert_eq!(clipboard.read(), Capture::Skipped(Skip::TooLarge));
    clipboard
        .write(
            &[Format::new(FormatKind::Text, "12345678")],
            Privacy::Normal,
        )
        .unwrap();
    assert!(matches!(clipboard.read(), Capture::Entry(_)));
}

#[test]
fn unknown_types_only_give_an_empty_capture() {
    let clipboard = Clipboard::unique();
    assert_eq!(clipboard.read(), Capture::Empty);
    clipboard.board.clearContents();
    assert!(clipboard.board.setData_forType(
        Some(&NSData::with_bytes(b"x")),
        &NSString::from_str("com.example.private")
    ));
    assert_eq!(clipboard.read(), Capture::Empty);
}

#[test]
fn single_image_file_link_also_stores_the_picture() {
    let dir = std::env::temp_dir().join(format!("pin-clip-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("photo.png");
    std::fs::write(&path, TINY_PNG).unwrap();
    let url = format!("file://{}\r\n", path.display());
    let clipboard = Clipboard::unique();
    clipboard.board.clearContents();
    assert!(clipboard.board.setData_forType(
        Some(&NSData::with_bytes(url.trim_end().as_bytes())),
        &NSString::from_str(FILE_URL)
    ));
    let formats = entry(clipboard.read());
    assert_eq!(kinds(&formats), [FormatKind::Png, FormatKind::Files]);
    assert_eq!(formats[0].data, TINY_PNG);
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn file_links_to_non_images_store_only_the_link() {
    let clipboard = Clipboard::unique();
    clipboard
        .write(
            &[Format::new(FormatKind::Files, "file:///etc/hosts\r\n")],
            Privacy::Normal,
        )
        .unwrap();
    assert_eq!(kinds(&entry(clipboard.read())), [FormatKind::Files]);
}

#[test]
fn clear_only_removes_the_expected_copy() {
    let clipboard = Clipboard::unique();
    let secret = clipboard
        .write(
            &[Format::new(FormatKind::Text, "s3cret")],
            Privacy::Concealed,
        )
        .unwrap();
    let newer = clipboard
        .write(&[Format::new(FormatKind::Text, "later")], Privacy::Normal)
        .unwrap();
    assert!(!clipboard.clear_if_unchanged(secret));
    assert!(matches!(clipboard.read(), Capture::Entry(_)));
    assert!(clipboard.clear_if_unchanged(newer));
    assert_eq!(clipboard.read(), Capture::Empty);
}

#[test]
fn writes_without_supported_formats_are_rejected() {
    let clipboard = Clipboard::unique();
    let other = Format::new(FormatKind::Other("x/y".into()), "z");
    assert!(matches!(
        clipboard.write(&[other], Privacy::Normal),
        Err(crate::Error::NothingToWrite)
    ));
}
