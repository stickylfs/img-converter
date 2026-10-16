use crate::codecs::{decode_with_image_crate, EncodeOptions};
use crate::codecs::{ImageDecoder, ImageEncoder};
use crate::decoded::DecodedImage;
use image::codecs::gif::GifEncoder;
use img_converter_common::{ConversionError, DecodeLimits, ImageFormat};

pub struct GifCodec;

impl ImageDecoder for GifCodec {
    /// Decodes the first frame of the GIF.
    fn decode(bytes: &[u8], limits: &DecodeLimits) -> Result<DecodedImage, ConversionError> {
        decode_with_image_crate(bytes, image::ImageFormat::Gif, ImageFormat::Gif, limits)
    }
}

impl ImageEncoder for GifCodec {
    /// Encodes a single frame, quantized to a 256-color palette.
    fn encode(image: &DecodedImage, _options: &EncodeOptions) -> Result<Vec<u8>, ConversionError> {
        let rgba = image.to_rgba_image().ok_or_else(|| {
            ConversionError::EncodeError("Invalid RGBA buffer for GIF encoding".to_string())
        })?;

        let mut buffer = Vec::new();
        {
            let mut encoder = GifEncoder::new_with_speed(&mut buffer, 10);
            encoder
                .encode_frame(image::Frame::new(rgba))
                .map_err(|e| ConversionError::EncodeError(e.to_string()))?;
        }
        Ok(buffer)
    }
}
