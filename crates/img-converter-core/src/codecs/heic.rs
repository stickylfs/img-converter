use crate::codecs::wic::decode_wic;
use crate::codecs::ImageDecoder;
use crate::decoded::DecodedImage;
use img_converter_common::{ConversionError, DecodeLimits, ImageFormat, WindowsCodec};

pub struct HeicCodec;

impl ImageDecoder for HeicCodec {
    /// Decodes HEIC/HEIF through the Windows Imaging Component, which requires the
    /// "HEIF Image Extensions" and "HEVC Video Extensions" from the Microsoft Store.
    fn decode(bytes: &[u8], limits: &DecodeLimits) -> Result<DecodedImage, ConversionError> {
        decode_wic(bytes, limits, ImageFormat::Heic).map_err(|e| match e {
            ConversionError::LimitExceeded(_) | ConversionError::MissingCodec(..) => e,
            other if !WindowsCodec::Heif.is_installed() => {
                ConversionError::MissingCodec(WindowsCodec::Heif, other.to_string())
            }
            // Not recognized although the HEIF decoder is installed: invalid file.
            other @ ConversionError::CorruptFile(_) => other,
            // Recognized but not decoded: usually the HEVC extension is missing.
            other => ConversionError::MissingCodec(WindowsCodec::Heif, other.to_string()),
        })
    }
}
