use objc2_app_kit::{NSBitmapImageFileType, NSBitmapImageRep};
use objc2_foundation::{NSData, NSDictionary, NSString};
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
    let clipboard = Clipboard::unique();
    let list = "file:///tmp/a%20b.txt\r\nfile:///tmp/c.txt\r\n";
    clipboard
        .write(&[Format::new(FormatKind::Files, list)], Privacy::Normal)
        .unwrap();
    let formats = entry(clipboard.read());
    assert_eq!(formats, [Format::new(FormatKind::Files, list)]);
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
    let clipboard = Clipboard::unique();
    clipboard
        .write(
            &[
                Format::new(FormatKind::Text, "a"),
                Format::new(FormatKind::Files, "file:///a\r\nfile:///b\r\n"),
            ],
            Privacy::Normal,
        )
        .unwrap();
    let items = clipboard.board.pasteboardItems().unwrap().to_vec();
    assert!(items[1].setData_forType(&NSData::new(), &NSString::from_str(CONCEALED)));
    assert_eq!(clipboard.read(), Capture::Skipped(Skip::Private));
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
fn writes_without_supported_formats_are_rejected() {
    let clipboard = Clipboard::unique();
    let other = Format::new(FormatKind::Other("x/y".into()), "z");
    assert!(matches!(
        clipboard.write(&[other], Privacy::Normal),
        Err(crate::Error::NothingToWrite)
    ));
}
