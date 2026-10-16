use crate::codecs::{decode_with_image_crate, write_with_encoder, EncodeOptions};
use crate::codecs::{ImageDecoder, ImageEncoder};
use crate::decoded::DecodedImage;
use image::codecs::tiff::TiffEncoder;
use img_converter_common::{ConversionError, DecodeLimits, ImageFormat};
use std::io::Cursor;

pub struct TiffCodec;

impl ImageDecoder for TiffCodec {
    fn decode(bytes: &[u8], limits: &DecodeLimits) -> Result<DecodedImage, ConversionError> {
        match decode_with_image_crate(bytes, image::ImageFormat::Tiff, ImageFormat::Tiff, limits) {
            Ok(decoded) => Ok(decoded),
            // Never retry an image that was rejected for being too large.
            Err(e @ ConversionError::LimitExceeded(_)) => Err(e),
            // Fall back to the Windows codec for TIFF variants the pure-Rust decoder
            // lacks (e.g. JPEG-compressed or planar layouts).
            Err(e) => {
                crate::codecs::wic::decode_wic(bytes, limits, ImageFormat::Tiff).map_err(|_| e)
            }
        }
    }
}

impl ImageEncoder for TiffCodec {
    fn encode(image: &DecodedImage, options: &EncodeOptions) -> Result<Vec<u8>, ConversionError> {
        let mut buffer = Cursor::new(Vec::new());
        write_with_encoder(TiffEncoder::new(&mut buffer), image, options, true)?;
        Ok(buffer.into_inner())
    }
}
