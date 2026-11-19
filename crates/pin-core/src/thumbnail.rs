use std::fmt;
use std::io::Cursor;

use image::{ImageFormat, ImageReader, Limits};

pub const MAX_WIDTH: u32 = 320;
pub const MAX_HEIGHT: u32 = 160;
const MAX_SOURCE_SIDE: u32 = 16_384;
const MAX_ALLOC: u64 = 256 * 1024 * 1024;

pub struct Thumbnail {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl fmt::Debug for Thumbnail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Thumbnail")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

pub(crate) fn encode(png: &[u8]) -> Option<Vec<u8>> {
    let mut reader = ImageReader::with_format(Cursor::new(png), ImageFormat::Png);
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SOURCE_SIDE);
    limits.max_image_height = Some(MAX_SOURCE_SIDE);
    limits.max_alloc = Some(MAX_ALLOC);
    reader.limits(limits);
    let full = reader.decode().ok()?;
    let small = if full.width() <= MAX_WIDTH && full.height() <= MAX_HEIGHT {
        full
    } else {
        full.thumbnail(MAX_WIDTH, MAX_HEIGHT)
    };
    let mut out = Cursor::new(Vec::new());
    small.to_rgba8().write_to(&mut out, ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

pub(crate) fn decode(png: &[u8]) -> Option<Thumbnail> {
    let image = image::load_from_memory_with_format(png, ImageFormat::Png).ok()?;
    let rgba = image.into_rgba8();
    Some(Thumbnail {
        width: rgba.width(),
        height: rgba.height(),
        rgba: rgba.into_raw(),
    })
}

pub(crate) fn png_size(png: &[u8]) -> Option<(u32, u32)> {
    const SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    let chunk = png.strip_prefix(SIGNATURE)?;
    if chunk.get(4..8)? != b"IHDR" {
        return None;
    }
    let header = chunk.get(8..16)?;
    let (width, height) = header.split_at(4);
    Some((
        u32::from_be_bytes(width.try_into().ok()?),
        u32::from_be_bytes(height.try_into().ok()?),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(width, height, image::Rgba([10, 20, 30, 255]));
        let mut out = Cursor::new(Vec::new());
        image.write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn thumbnail_fits_bounds_and_keeps_aspect() {
        let thumb = decode(&encode(&png(1000, 250)).unwrap()).unwrap();
        assert_eq!((thumb.width, thumb.height), (MAX_WIDTH, 80));
        assert_eq!(thumb.rgba.len(), 320 * 80 * 4);
    }

    #[test]
    fn small_images_are_not_upscaled() {
        let thumb = decode(&encode(&png(40, 20)).unwrap()).unwrap();
        assert_eq!((thumb.width, thumb.height), (40, 20));
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(encode(b"not a png").is_none());
        assert!(png_size(b"nope").is_none());
    }

    #[test]
    fn png_size_reads_header() {
        assert_eq!(png_size(&png(7, 3)), Some((7, 3)));
    }
}
