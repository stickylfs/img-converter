use crate::codecs::{decode_with_image_crate, write_with_encoder, EncodeOptions};
use crate::codecs::{ImageDecoder, ImageEncoder};
use crate::decoded::DecodedImage;
use image::codecs::jpeg::JpegEncoder;
use img_converter_common::{ConversionError, DecodeLimits, ImageFormat};

pub struct JpegCodec;

impl ImageDecoder for JpegCodec {
    fn decode(bytes: &[u8], limits: &DecodeLimits) -> Result<DecodedImage, ConversionError> {
        decode_with_image_crate(bytes, image::ImageFormat::Jpeg, ImageFormat::Jpeg, limits)
    }
}

impl ImageEncoder for JpegCodec {
    fn encode(image: &DecodedImage, options: &EncodeOptions) -> Result<Vec<u8>, ConversionError> {
        let mut buffer = Vec::new();
        let encoder = JpegEncoder::new_with_quality(&mut buffer, options.quality.clamp(1, 100));
        write_with_encoder(encoder, image, options, false)?;
        Ok(buffer)
    }
}
