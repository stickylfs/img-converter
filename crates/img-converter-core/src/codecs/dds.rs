use crate::codecs::{check_decoded, ImageDecoder};
use crate::decoded::{DecodedImage, PixelFormat};
use img_converter_common::{ConversionError, DecodeLimits, ImageFormat};
use std::io::Cursor;

pub struct DdsCodec;

impl ImageDecoder for DdsCodec {
    /// Decodes the first mipmap of a DirectDraw Surface texture.
    fn decode(bytes: &[u8], limits: &DecodeLimits) -> Result<DecodedImage, ConversionError> {
        limits
            .check_file_size(bytes.len() as u64)
            .map_err(ConversionError::LimitExceeded)?;

        let dds = image_dds::ddsfile::Dds::read(&mut Cursor::new(bytes))
            .map_err(|e| ConversionError::DecodeError(format!("Failed to parse DDS: {}", e)))?;

        // Reject oversized textures from the header, before decompressing.
        limits
            .check_dimensions(dds.get_width(), dds.get_height())
            .map_err(ConversionError::LimitExceeded)?;

        let rgba = image_dds::image_from_dds(&dds, 0)
            .map_err(|e| ConversionError::DecodeError(format!("Failed to decode DDS: {}", e)))?;

        let (width, height) = rgba.dimensions();
        let decoded = DecodedImage::new(
            width,
            height,
            PixelFormat::Rgba8,
            rgba.into_raw(),
            ImageFormat::Dds,
        );
        check_decoded(&decoded, limits)?;
        Ok(decoded)
    }
}
