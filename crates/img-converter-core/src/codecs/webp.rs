use crate::codecs::{decode_with_image_crate, write_with_encoder, EncodeOptions};
use crate::codecs::{ImageDecoder, ImageEncoder};
use crate::decoded::DecodedImage;
use image::codecs::webp::WebPEncoder;
use img_converter_common::{ConversionError, DecodeLimits, ImageFormat};

pub struct WebpCodec;

impl ImageDecoder for WebpCodec {
    fn decode(bytes: &[u8], limits: &DecodeLimits) -> Result<DecodedImage, ConversionError> {
        decode_with_image_crate(bytes, image::ImageFormat::WebP, ImageFormat::Webp, limits)
    }
}

impl ImageEncoder for WebpCodec {
    /// The pure-Rust WebP encoder only supports lossless output, so `quality` is unused.
    fn encode(image: &DecodedImage, options: &EncodeOptions) -> Result<Vec<u8>, ConversionError> {
        let mut buffer = Vec::new();
        write_with_encoder(WebPEncoder::new_lossless(&mut buffer), image, options, true)?;
        Ok(buffer)
    }
}
