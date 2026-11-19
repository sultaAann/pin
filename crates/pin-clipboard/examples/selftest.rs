//! Writes test entries to the general pasteboard with pin's writer, checks what other apps
//! would paste, then restores the original clipboard.
#![cfg_attr(
    target_os = "macos",
    expect(clippy::print_stdout, clippy::unwrap_used, reason = "diagnostic tool")
)]

#[cfg(target_os = "macos")]
fn main() {
    use std::process::Command;

    use objc2_app_kit::NSPasteboard;
    use pin_clipboard::{Capture, Clipboard, Privacy};
    use pin_core::{Format, FormatKind};

    fn types() -> Vec<String> {
        let board = NSPasteboard::generalPasteboard();
        let items = board
            .pasteboardItems()
            .map(|i| i.to_vec())
            .unwrap_or_default();
        items
            .iter()
            .flat_map(|item| item.types().to_vec())
            .map(|t| t.to_string())
            .collect()
    }

    let clipboard = Clipboard::general().unwrap();
    let original = match clipboard.read() {
        Capture::Entry(entry) => Some(entry.formats),
        _ => None,
    };

    clipboard
        .write(
            &[
                Format::new(FormatKind::Text, "pin selftest text"),
                Format::new(FormatKind::Html, "<b>pin selftest</b>"),
            ],
            Privacy::Normal,
        )
        .unwrap();
    let pasted = Command::new("pbpaste").output().unwrap().stdout;
    println!("text: pbpaste ok = {}", pasted == b"pin selftest text");
    println!("text types: {:?}", types());

    let png = std::fs::read(std::env::args().nth(1).unwrap_or_default()).ok();
    if let Some(png) = png {
        clipboard
            .write(&[Format::new(FormatKind::Png, png)], Privacy::Normal)
            .unwrap();
        println!("image types: {:?}", types());
    }

    clipboard
        .write(
            &[Format::new(
                FormatKind::Files,
                "file:///System/Library/Fonts/SFNS.ttf\r\n",
            )],
            Privacy::Normal,
        )
        .unwrap();
    println!("files types: {:?}", types());

    match original {
        Some(formats) => {
            clipboard.write(&formats, Privacy::Normal).unwrap();
            println!("restored original clipboard ({} formats)", formats.len());
        }
        None => println!("original clipboard was empty or skipped; not restored"),
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {}
