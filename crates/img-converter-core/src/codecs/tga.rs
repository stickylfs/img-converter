use crate::codecs::{decode_with_image_crate, write_with_encoder, EncodeOptions};
use crate::codecs::{ImageDecoder, ImageEncoder};
use crate::decoded::DecodedImage;
use image::codecs::tga::TgaEncoder;
use img_converter_common::{ConversionError, DecodeLimits, ImageFormat};

pub struct TgaCodec;

impl ImageDecoder for TgaCodec {
    fn decode(bytes: &[u8], limits: &DecodeLimits) -> Result<DecodedImage, ConversionError> {
        decode_with_image_crate(bytes, image::ImageFormat::Tga, ImageFormat::Tga, limits)
    }
}

impl ImageEncoder for TgaCodec {
    fn encode(image: &DecodedImage, options: &EncodeOptions) -> Result<Vec<u8>, ConversionError> {
        let mut buffer = Vec::new();
        write_with_encoder(TgaEncoder::new(&mut buffer), image, options, true)?;
        Ok(buffer)
    }
}
