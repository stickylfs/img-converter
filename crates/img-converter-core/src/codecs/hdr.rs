use crate::codecs::{decode_with_image_crate, EncodeOptions};
use crate::codecs::{ImageDecoder, ImageEncoder};
use crate::color::srgb_u8_to_linear;
use crate::decoded::DecodedImage;
use image::codecs::hdr::HdrEncoder;
use image::Rgb;
use img_converter_common::{ConversionError, DecodeLimits, ImageFormat};

pub struct HdrCodec;

impl ImageDecoder for HdrCodec {
    /// Decodes Radiance RGBE; linear values above 1.0 are clipped when converted to sRGB.
    fn decode(bytes: &[u8], limits: &DecodeLimits) -> Result<DecodedImage, ConversionError> {
        decode_with_image_crate(bytes, image::ImageFormat::Hdr, ImageFormat::Hdr, limits)
    }
}

impl ImageEncoder for HdrCodec {
    fn encode(image: &DecodedImage, _options: &EncodeOptions) -> Result<Vec<u8>, ConversionError> {
        // Radiance HDR stores linear light, so undo the sRGB transfer curve.
        let pixels: Vec<Rgb<f32>> = image
            .to_rgb8_raw()
            .chunks_exact(3)
            .map(|c| {
                Rgb([
                    srgb_u8_to_linear(c[0]),
                    srgb_u8_to_linear(c[1]),
                    srgb_u8_to_linear(c[2]),
                ])
            })
            .collect();

        let mut buffer = Vec::new();
        HdrEncoder::new(&mut buffer)
            .encode(&pixels, image.width as usize, image.height as usize)
            .map_err(|e| ConversionError::EncodeError(e.to_string()))?;
        Ok(buffer)
    }
}
