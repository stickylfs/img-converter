use crate::codecs::{decode_with_image_crate, EncodeOptions};
use crate::codecs::{ImageDecoder, ImageEncoder};
use crate::color::srgb_u8_to_linear;
use crate::decoded::DecodedImage;
use image::codecs::openexr::OpenExrEncoder;
use image::{ExtendedColorType, ImageEncoder as _};
use img_converter_common::{ConversionError, DecodeLimits, ImageFormat};
use std::io::Cursor;

pub struct ExrCodec;

impl ImageDecoder for ExrCodec {
    /// Decodes OpenEXR; linear values above 1.0 are clipped when converted to sRGB.
    fn decode(bytes: &[u8], limits: &DecodeLimits) -> Result<DecodedImage, ConversionError> {
        decode_with_image_crate(bytes, image::ImageFormat::OpenExr, ImageFormat::Exr, limits)
    }
}

impl ImageEncoder for ExrCodec {
    fn encode(image: &DecodedImage, _options: &EncodeOptions) -> Result<Vec<u8>, ConversionError> {
        // OpenEXR stores linear light: undo the sRGB curve on color channels only.
        let (source, channels, color) = if image.has_alpha() {
            (image.to_rgba8(), 4, ExtendedColorType::Rgba32F)
        } else {
            (image.to_rgb8_raw(), 3, ExtendedColorType::Rgb32F)
        };

        let mut f32_bytes = Vec::with_capacity(source.len() * 4);
        for (i, &value) in source.iter().enumerate() {
            let linear = if channels == 4 && i % 4 == 3 {
                value as f32 / 255.0
            } else {
                srgb_u8_to_linear(value)
            };
            f32_bytes.extend_from_slice(&linear.to_ne_bytes());
        }

        let mut buffer = Cursor::new(Vec::new());
        OpenExrEncoder::new(&mut buffer)
            .write_image(&f32_bytes, image.width, image.height, color)
            .map_err(|e| ConversionError::EncodeError(e.to_string()))?;
        Ok(buffer.into_inner())
    }
}
