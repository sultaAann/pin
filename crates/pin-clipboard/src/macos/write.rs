use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{NSPasteboard, NSPasteboardItem, NSPasteboardWriting};
use objc2_foundation::{NSArray, NSData, NSString};
use pin_core::{Format, FormatKind};

use super::{CONCEALED, FILE_URL, HTML, PNG, RTF, TEXT};
use crate::{Error, Privacy, Result};

pub(super) fn write(board: &NSPasteboard, formats: &[Format], privacy: Privacy) -> Result<()> {
    let mut items = vec![NSPasteboardItem::new()];
    let mut written = false;
    for format in formats {
        let uti = match &format.kind {
            FormatKind::Text => TEXT,
            FormatKind::Html => HTML,
            FormatKind::Rtf => RTF,
            FormatKind::Png => PNG,
            FormatKind::Files => {
                written |= write_files(&mut items, &format.data)?;
                continue;
            }
            FormatKind::Other(_) => continue,
        };
        set(&items[0], uti, &format.data)?;
        written = true;
    }
    if !written {
        return Err(Error::NothingToWrite);
    }
    if privacy == Privacy::Concealed {
        set(&items[0], CONCEALED, &[])?;
    }
    let objects: Vec<Retained<ProtocolObject<dyn NSPasteboardWriting>>> = items
        .into_iter()
        .map(ProtocolObject::from_retained)
        .collect();
    board.clearContents();
    if board.writeObjects(&NSArray::from_retained_slice(&objects)) {
        Ok(())
    } else {
        Err(Error::WriteRejected)
    }
}

fn write_files(items: &mut Vec<Retained<NSPasteboardItem>>, list: &[u8]) -> Result<bool> {
    let Ok(list) = std::str::from_utf8(list) else {
        return Ok(false);
    };
    // A link to a deleted or unmounted file would paste nothing, so it is left out.
    let uris = list.lines().map(str::trim).filter(|line| {
        line.starts_with("file:")
            && super::file_path(line.as_bytes()).is_some_and(|path| path.symlink_metadata().is_ok())
    });
    let mut written = false;
    for (index, uri) in uris.enumerate() {
        if index >= items.len() {
            items.push(NSPasteboardItem::new());
        }
        set(&items[index], FILE_URL, uri.as_bytes())?;
        written = true;
    }
    Ok(written)
}

fn set(item: &NSPasteboardItem, uti: &str, data: &[u8]) -> Result<()> {
    if item.setData_forType(&NSData::with_bytes(data), &NSString::from_str(uti)) {
        Ok(())
    } else {
        Err(Error::WriteRejected)
    }
}
