use crate::codecs::wic::decode_wic;
use crate::codecs::ImageDecoder;
use crate::decoded::DecodedImage;
use crate::orientation::orientation_from_container;
use image::ImageReader;
use img_converter_common::{ConversionError, DecodeLimits, ImageFormat, WindowsCodec};
use std::io::Cursor;

/// Embedded previews smaller than this are thumbnails, not usable conversions.
const MIN_PREVIEW_PIXELS: u64 = 1_000_000;

pub struct RawCodec;

impl ImageDecoder for RawCodec {
    /// Decodes camera RAW files, trying in order:
    /// 1. the Windows RAW codec through WIC (built into Windows 10 1903+ and
    ///    Windows 11, kept up to date by the "Raw Image Extension"),
    /// 2. the full-size JPEG preview embedded by the camera.
    fn decode(bytes: &[u8], limits: &DecodeLimits) -> Result<DecodedImage, ConversionError> {
        limits
            .check_file_size(bytes.len() as u64)
            .map_err(ConversionError::LimitExceeded)?;

        let wic_error = match decode_wic(bytes, limits, ImageFormat::Raw) {
            Ok(decoded) => return Ok(decoded),
            Err(e @ ConversionError::LimitExceeded(_)) => return Err(e),
            Err(e) => e,
        };

        if let Some(mut preview) = extract_embedded_preview(bytes, limits)? {
            preview.orientation = orientation_from_container(bytes);
            return Ok(preview);
        }

        match wic_error {
            // The codec is installed but does not recognize the file: invalid file.
            e @ ConversionError::CorruptFile(_) if WindowsCodec::Raw.is_installed() => Err(e),
            e @ ConversionError::MissingCodec(..) => Err(e),
            // Missing codec, or a camera newer than the installed codec.
            e => Err(ConversionError::MissingCodec(
                WindowsCodec::Raw,
                e.to_string(),
            )),
        }
    }
}

/// Finds the largest embedded JPEG stream (camera-rendered preview) and decodes it.
fn extract_embedded_preview(
    bytes: &[u8],
    limits: &DecodeLimits,
) -> Result<Option<DecodedImage>, ConversionError> {
    let mut best: Option<(u64, usize)> = None;

    let mut i = 0;
    while i + 3 < bytes.len() {
        if bytes[i] == 0xFF && bytes[i + 1] == 0xD8 && bytes[i + 2] == 0xFF {
            // Only parse the header here; the full decode happens once for the winner.
            let reader =
                ImageReader::with_format(Cursor::new(&bytes[i..]), image::ImageFormat::Jpeg);
            if let Ok((w, h)) = reader.into_dimensions() {
                let pixels = w as u64 * h as u64;
                if best.is_none_or(|(best_pixels, _)| pixels > best_pixels) {
                    best = Some((pixels, i));
                }
            }
        }
        i += 1;
    }

    let Some((_, offset)) = best.filter(|&(pixels, _)| pixels >= MIN_PREVIEW_PIXELS) else {
        return Ok(None);
    };

    match crate::codecs::decode_with_image_crate(
        &bytes[offset..],
        image::ImageFormat::Jpeg,
        ImageFormat::Raw,
        limits,
    ) {
        Ok(decoded) => Ok(Some(decoded)),
        Err(e @ ConversionError::LimitExceeded(_)) => Err(e),
        Err(_) => Ok(None),
    }
}
