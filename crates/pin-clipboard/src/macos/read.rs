use objc2::rc::Retained;
use objc2_app_kit::{
    NSBitmapImageFileType, NSBitmapImageRep, NSPasteboard, NSPasteboardItem, NSWorkspace,
};
use objc2_foundation::{NSData, NSDictionary, NSString};
use pin_core::{Format, FormatKind, NewEntry};

use super::{AUTO_GENERATED, CONCEALED, FILE_URL, HTML, PNG, RTF, TEXT, TIFF, TRANSIENT};
use crate::{Capture, Skip};

const DIRECT: [(&str, FormatKind); 4] = [
    (TEXT, FormatKind::Text),
    (HTML, FormatKind::Html),
    (RTF, FormatKind::Rtf),
    (PNG, FormatKind::Png),
];

struct TooLarge;

struct Budget(usize);

impl Budget {
    fn check(&self, data: &NSData) -> Result<(), TooLarge> {
        if data.len() > self.0 {
            Err(TooLarge)
        } else {
            Ok(())
        }
    }

    fn take(&mut self, data: &NSData) -> Result<Vec<u8>, TooLarge> {
        self.0 = self.0.checked_sub(data.len()).ok_or(TooLarge)?;
        Ok(data.to_vec())
    }
}

pub(super) fn read(board: &NSPasteboard, limit: usize) -> Capture {
    let items = board
        .pasteboardItems()
        .map(|items| items.to_vec())
        .unwrap_or_default();
    let Some(first) = items.first() else {
        return Capture::Empty;
    };
    if items.iter().any(|item| is_private(&item_types(item))) {
        return Capture::Skipped(Skip::Private);
    }
    let types = item_types(first);
    match formats(first, &types, &items, &mut Budget(limit)) {
        Ok(formats) if formats.is_empty() => Capture::Empty,
        Ok(formats) => Capture::Entry(NewEntry {
            formats,
            source_app: frontmost_app(),
        }),
        Err(TooLarge) => Capture::Skipped(Skip::TooLarge),
    }
}

fn formats(
    first: &NSPasteboardItem,
    types: &[String],
    items: &[Retained<NSPasteboardItem>],
    budget: &mut Budget,
) -> Result<Vec<Format>, TooLarge> {
    let mut formats = Vec::new();
    for (uti, kind) in DIRECT {
        if has(types, uti)
            && let Some(data) = first.dataForType(&NSString::from_str(uti))
        {
            formats.push(Format::new(kind, budget.take(&data)?));
        }
    }
    if !has(types, PNG)
        && has(types, TIFF)
        && let Some(tiff) = first.dataForType(&NSString::from_str(TIFF))
    {
        budget.check(&tiff)?;
        if let Some(png) = tiff_to_png(&tiff) {
            formats.push(Format::new(FormatKind::Png, budget.take(&png)?));
        }
    }
    if let Some(files) = file_list(items, budget)? {
        formats.push(Format::new(FormatKind::Files, files));
    }
    Ok(formats)
}

fn file_list(
    items: &[Retained<NSPasteboardItem>],
    budget: &mut Budget,
) -> Result<Option<Vec<u8>>, TooLarge> {
    let uti = NSString::from_str(FILE_URL);
    let mut list = Vec::new();
    for item in items {
        if let Some(data) = item.dataForType(&uti) {
            let url = budget.take(&data)?;
            let url = url.strip_suffix(&[0]).unwrap_or(&url);
            if url.starts_with(b"file:") {
                list.extend_from_slice(url);
                list.extend_from_slice(b"\r\n");
            }
        }
    }
    Ok((!list.is_empty()).then_some(list))
}

fn tiff_to_png(tiff: &NSData) -> Option<Retained<NSData>> {
    let rep = NSBitmapImageRep::imageRepWithData(tiff)?;
    // SAFETY: an empty dictionary is a valid properties argument; it holds no values whose type could mismatch.
    unsafe {
        rep.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
}

fn item_types(item: &NSPasteboardItem) -> Vec<String> {
    item.types()
        .to_vec()
        .iter()
        .map(ToString::to_string)
        .collect()
}

fn is_private(types: &[String]) -> bool {
    [CONCEALED, TRANSIENT, AUTO_GENERATED]
        .iter()
        .any(|marker| has(types, marker))
}

fn has(types: &[String], uti: &str) -> bool {
    types.iter().any(|t| t == uti)
}

fn frontmost_app() -> Option<String> {
    NSWorkspace::sharedWorkspace()
        .frontmostApplication()?
        .localizedName()
        .map(|name| name.to_string())
}
