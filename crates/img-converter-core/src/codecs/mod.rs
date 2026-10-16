pub mod avif;
pub mod bmp;
pub mod dds;
pub mod exr;
pub mod gif;
pub mod hdr;
pub mod heic;
pub mod ico;
pub mod jpeg;
pub mod pdf;
pub mod png;
pub mod raw;
pub mod svg;
pub mod tga;
pub mod tiff;
pub mod webp;
pub mod wic;

use crate::decoded::DecodedImage;
use image::{ExtendedColorType, ImageDecoder as _, ImageReader};
use img_converter_common::{ConversionError, DecodeLimits, ImageFormat, MetadataPolicy};
use std::io::Cursor;

/// Encoder settings shared by every output format.
#[derive(Debug, Clone, Copy)]
pub struct EncodeOptions {
    /// Quality (1-100) for lossy formats.
    pub quality: u8,
    /// AVIF encoder speed (1 = slowest / smallest, 10 = fastest).
    pub avif_speed: u8,
    /// Bundle 16-256 px layers into ICO output.
    pub ico_multi_resolution: bool,
    pub metadata: MetadataPolicy,
}

impl Default for EncodeOptions {
    fn default() -> Self {
        Self {
            quality: 90,
            avif_speed: 6,
            ico_multi_resolution: false,
            metadata: MetadataPolicy::Preserve,
        }
    }
}

/// Trait implemented by format-specific image decoders.
pub trait ImageDecoder {
    fn decode(bytes: &[u8], limits: &DecodeLimits) -> Result<DecodedImage, ConversionError>;
}

/// Trait implemented by format-specific image encoders.
pub trait ImageEncoder {
    fn encode(image: &DecodedImage, options: &EncodeOptions) -> Result<Vec<u8>, ConversionError>;
}

/// Maps our [`DecodeLimits`] onto the `image` crate limits so oversized images are
/// rejected from their header, before any pixel buffer is allocated.
fn image_limits(limits: &DecodeLimits) -> image::Limits {
    let mut out = image::Limits::default();
    out.max_image_width = Some(limits.max_width);
    out.max_image_height = Some(limits.max_height);
    out.max_alloc = Some(limits.max_memory_bytes);
    out
}

fn decode_error(e: image::ImageError) -> ConversionError {
    match e {
        image::ImageError::Limits(l) => ConversionError::LimitExceeded(l.to_string()),
        other => ConversionError::DecodeError(other.to_string()),
    }
}

/// Validates the decoded size against `limits`.
pub(crate) fn check_decoded(
    image: &DecodedImage,
    limits: &DecodeLimits,
) -> Result<(), ConversionError> {
    limits
        .check_dimensions(image.width, image.height)
        .map_err(ConversionError::LimitExceeded)?;
    limits
        .check_memory_bytes(image.pixels.len() as u64)
        .map_err(ConversionError::LimitExceeded)
}

/// Decodes `bytes` with the `image` crate, keeping ICC profile, EXIF block and
/// EXIF orientation so they can be applied or preserved later in the pipeline.
pub(crate) fn decode_with_image_crate(
    bytes: &[u8],
    format: image::ImageFormat,
    source_format: ImageFormat,
    limits: &DecodeLimits,
) -> Result<DecodedImage, ConversionError> {
    limits
        .check_file_size(bytes.len() as u64)
        .map_err(ConversionError::LimitExceeded)?;

    let mut reader = ImageReader::with_format(Cursor::new(bytes), format);
    reader.limits(image_limits(limits));
    let mut decoder = reader.into_decoder().map_err(decode_error)?;

    let (width, height) = decoder.dimensions();
    limits
        .check_dimensions(width, height)
        .map_err(ConversionError::LimitExceeded)?;

    let icc_profile = decoder.icc_profile().ok().flatten();
    let exif_data = decoder.exif_metadata().ok().flatten();
    let orientation = decoder.orientation().map(|o| o.to_exif()).unwrap_or(1);

    let dynamic = image::DynamicImage::from_decoder(decoder).map_err(decode_error)?;
    let mut decoded = DecodedImage::from_dynamic(dynamic, source_format);
    decoded.icc_profile = icc_profile;
    decoded.exif_data = exif_data;
    decoded.orientation = orientation;

    check_decoded(&decoded, limits)?;
    Ok(decoded)
}

/// Writes `image` through an `image` crate encoder, embedding ICC/EXIF metadata
/// when requested and supported by the format.
pub(crate) fn write_with_encoder<E: image::ImageEncoder>(
    mut encoder: E,
    image: &DecodedImage,
    options: &EncodeOptions,
    keep_alpha: bool,
) -> Result<(), ConversionError> {
    if options.metadata == MetadataPolicy::Preserve {
        // Unsupported metadata kinds are silently skipped.
        if let Some(icc) = &image.icc_profile {
            let _ = encoder.set_icc_profile(icc.clone());
        }
        if let Some(exif) = &image.exif_data {
            let _ = encoder.set_exif_metadata(exif.clone());
        }
    }

    let (data, color) = if keep_alpha && image.has_alpha() {
        (image.to_rgba8(), ExtendedColorType::Rgba8)
    } else {
        (image.to_rgb8_raw(), ExtendedColorType::Rgb8)
    };

    encoder
        .write_image(&data, image.width, image.height, color)
        .map_err(|e| ConversionError::EncodeError(e.to_string()))
}
