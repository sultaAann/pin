use objc2::rc::Retained;
use objc2_app_kit::{
    NSBitmapImageFileType, NSBitmapImageRep, NSPasteboard, NSPasteboardItem, NSWorkspace,
};
use objc2_foundation::{NSData, NSDictionary, NSString, NSURL};
use pin_core::{Format, FormatKind, NewEntry};

use super::{
    AUTO_GENERATED, CONCEALED, FILE_URL, HEIC, HTML, JPEG, PNG, RTF, TEXT, TIFF, TRANSIENT,
};
use crate::{Capture, Skip};

const REFERENCE_PREFIX: &[u8] = b"file:///.file/id=";

const ENCODED_IMAGES: [&str; 3] = [TIFF, JPEG, HEIC];

const IMAGE_EXTENSIONS: [&str; 11] = [
    "png", "jpg", "jpeg", "heic", "heif", "gif", "tif", "tiff", "bmp", "webp", "avif",
];

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
        self.reserve(data.len())?;
        Ok(data.to_vec())
    }

    fn reserve(&mut self, len: usize) -> Result<(), TooLarge> {
        self.0 = self.0.checked_sub(len).ok_or(TooLarge)?;
        Ok(())
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
    let files = file_list(items, budget)?;
    for (uti, kind) in DIRECT {
        if has(types, uti)
            && let Some(data) = first.dataForType(&NSString::from_str(uti))
        {
            formats.push(Format::new(kind, budget.take(&data)?));
        }
    }
    if !has(types, PNG) {
        match &files {
            // Finder puts the file icon on the pasteboard as TIFF; that is not the copied content.
            Some(files) => {
                if let Some(png) = single_image_file(files, budget)? {
                    formats.push(Format::new(FormatKind::Png, png));
                }
            }
            None => {
                if let Some(png) = encoded_image(first, types, budget)? {
                    formats.push(Format::new(FormatKind::Png, png));
                }
            }
        }
    }
    if let Some(files) = files {
        formats.push(Format::new(FormatKind::Files, files));
    }
    Ok(formats)
}

/// Images that arrive only as TIFF, JPEG or HEIC data (screenshots, Preview, Photos), as PNG.
fn encoded_image(
    first: &NSPasteboardItem,
    types: &[String],
    budget: &mut Budget,
) -> Result<Option<Vec<u8>>, TooLarge> {
    for uti in ENCODED_IMAGES {
        if has(types, uti)
            && let Some(data) = first.dataForType(&NSString::from_str(uti))
        {
            budget.check(&data)?;
            if let Some(png) = to_png(&data) {
                return budget.take(&png).map(Some);
            }
        }
    }
    Ok(None)
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
                list.extend_from_slice(path_url(url).as_deref().unwrap_or(url));
                list.extend_from_slice(b"\r\n");
            }
        }
    }
    Ok((!list.is_empty()).then_some(list))
}

// Finder copies file reference URLs (file:///.file/id=…): they name no path to show or search,
// and they stop resolving on another volume, so they are stored as the path they point to.
fn path_url(url: &[u8]) -> Option<Vec<u8>> {
    if !url.starts_with(REFERENCE_PREFIX) {
        return None;
    }
    let url = NSURL::URLWithString(&NSString::from_str(std::str::from_utf8(url).ok()?))?;
    Some(
        url.filePathURL()?
            .absoluteString()?
            .to_string()
            .into_bytes(),
    )
}

// Apps like Telegram copy an image as a link to a file only; the picture itself is kept so it
// still pastes into apps that want image data, and after the source file is gone.
fn single_image_file(list: &[u8], budget: &mut Budget) -> Result<Option<Vec<u8>>, TooLarge> {
    let mut urls = list
        .split(|&b| b == b'\n')
        .map(<[u8]>::trim_ascii)
        .filter(|u| !u.is_empty());
    let (Some(url), None) = (urls.next(), urls.next()) else {
        return Ok(None);
    };
    let Some(path) = super::file_path(url) else {
        return Ok(None);
    };
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if !IMAGE_EXTENSIONS.contains(&extension.as_str()) {
        return Ok(None);
    }
    let Ok(meta) = std::fs::metadata(&path) else {
        return Ok(None);
    };
    if !meta.is_file() || usize::try_from(meta.len()).map_or(true, |len| len > budget.0) {
        return Ok(None);
    }
    let Ok(bytes) = std::fs::read(&path) else {
        return Ok(None);
    };
    let png = if extension == "png" {
        bytes
    } else {
        match to_png(&NSData::with_bytes(&bytes)) {
            Some(png) => png.to_vec(),
            None => return Ok(None),
        }
    };
    budget.reserve(png.len())?;
    Ok(Some(png))
}

fn to_png(image: &NSData) -> Option<Retained<NSData>> {
    let rep = NSBitmapImageRep::imageRepWithData(image)?;
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
