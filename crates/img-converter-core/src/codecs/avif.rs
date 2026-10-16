use crate::codecs::wic::decode_wic;
use crate::codecs::EncodeOptions;
use crate::codecs::{ImageDecoder, ImageEncoder};
use crate::decoded::DecodedImage;
use img_converter_common::{ConversionError, DecodeLimits, ImageFormat, WindowsCodec};
use imgref::Img;
use rgb::{RGB8, RGBA8};

pub struct AvifCodec;

impl ImageDecoder for AvifCodec {
    /// Decodes AVIF through the Windows Imaging Component, which requires the
    /// "AV1 Video Extension" (preinstalled on Windows 11, free in the Microsoft
    /// Store for Windows 10).
    fn decode(bytes: &[u8], limits: &DecodeLimits) -> Result<DecodedImage, ConversionError> {
        decode_wic(bytes, limits, ImageFormat::Avif).map_err(|e| match e {
            ConversionError::LimitExceeded(_) | ConversionError::MissingCodec(..) => e,
            other if !WindowsCodec::Av1.is_installed() => {
                ConversionError::MissingCodec(WindowsCodec::Av1, other.to_string())
            }
            other => other,
        })
    }
}

impl ImageEncoder for AvifCodec {
    /// Encodes with the pure-Rust `ravif` (rav1e) encoder.
    ///
    /// Output is 8-bit: ravif defaults to 10-bit, which the Windows AV1 decoder
    /// (used by Photos, Explorer thumbnails and our own AVIF input) rejects.
    fn encode(image: &DecodedImage, options: &EncodeOptions) -> Result<Vec<u8>, ConversionError> {
        let encoder = ravif::Encoder::new()
            .with_bit_depth(ravif::BitDepth::Eight)
            .with_quality(options.quality.clamp(1, 100) as f32)
            .with_speed(options.avif_speed.clamp(1, 10));
        let (width, height) = (image.width as usize, image.height as usize);

        let encoded = if image.has_alpha() {
            let pixels: Vec<RGBA8> = image
                .to_rgba8()
                .chunks_exact(4)
                .map(|c| RGBA8::new(c[0], c[1], c[2], c[3]))
                .collect();
            encoder.encode_rgba(Img::new(pixels.as_slice(), width, height))
        } else {
            let pixels: Vec<RGB8> = image
                .to_rgb8_raw()
                .chunks_exact(3)
                .map(|c| RGB8::new(c[0], c[1], c[2]))
                .collect();
            encoder.encode_rgb(Img::new(pixels.as_slice(), width, height))
        };

        encoded
            .map(|e| e.avif_file)
            .map_err(|e| ConversionError::EncodeError(e.to_string()))
    }
}
