use crate::codecs::{decode_with_image_crate, write_with_encoder, EncodeOptions};
use crate::codecs::{ImageDecoder, ImageEncoder};
use crate::decoded::DecodedImage;
use image::codecs::bmp::BmpEncoder;
use img_converter_common::{ConversionError, DecodeLimits, ImageFormat};

pub struct BmpCodec;

impl ImageDecoder for BmpCodec {
    fn decode(bytes: &[u8], limits: &DecodeLimits) -> Result<DecodedImage, ConversionError> {
        decode_with_image_crate(bytes, image::ImageFormat::Bmp, ImageFormat::Bmp, limits)
    }
}

impl ImageEncoder for BmpCodec {
    fn encode(image: &DecodedImage, options: &EncodeOptions) -> Result<Vec<u8>, ConversionError> {
        let mut buffer = Vec::new();
        write_with_encoder(BmpEncoder::new(&mut buffer), image, options, false)?;
        Ok(buffer)
    }
}
