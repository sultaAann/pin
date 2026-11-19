//! Prints the type identifiers and sizes on the general pasteboard, never its contents.
#![cfg_attr(
    target_os = "macos",
    expect(clippy::print_stdout, reason = "diagnostic tool")
)]

#[cfg(target_os = "macos")]
fn main() {
    use objc2_app_kit::NSPasteboard;

    let board = NSPasteboard::generalPasteboard();
    println!("changeCount {}", board.changeCount());
    let items = board
        .pasteboardItems()
        .map(|i| i.to_vec())
        .unwrap_or_default();
    for (index, item) in items.iter().enumerate() {
        println!("item {index}");
        for uti in item.types().to_vec() {
            let size = item.dataForType(&uti).map_or(0, |d| d.len());
            println!("  {uti} {size} B");
        }
    }
    match pin_clipboard::Clipboard::general().map(|c| c.read()) {
        Ok(pin_clipboard::Capture::Entry(entry)) => {
            for format in &entry.formats {
                println!(
                    "pin stores {} {} B",
                    format.kind.as_str(),
                    format.data.len()
                );
            }
        }
        other => println!(
            "pin capture: {:?}",
            other.map(|c| matches!(c, pin_clipboard::Capture::Empty))
        ),
    }
}

#[cfg(not(target_os = "macos"))]
fn main() {}
