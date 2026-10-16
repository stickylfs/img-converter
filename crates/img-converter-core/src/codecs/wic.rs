use crate::decoded::{DecodedImage, PixelFormat};
use img_converter_common::{ConversionError, DecodeLimits, ImageFormat, WindowsCodec};
use windows::Win32::Graphics::Imaging::*;
use windows::Win32::System::Com::*;

/// The optional Windows codec that decodes `format`, if any.
fn optional_codec(format: ImageFormat) -> Option<WindowsCodec> {
    match format {
        ImageFormat::Avif => Some(WindowsCodec::Av1),
        ImageFormat::Heic => Some(WindowsCodec::Heif),
        ImageFormat::Raw => Some(WindowsCodec::Raw),
        _ => None,
    }
}

/// Decodes image data using the Windows Imaging Component (WIC).
///
/// This provides hardware-accelerated and OS-native support for formats like
/// HEIC/HEIF, Adobe DNG, Camera RAW, and other Windows codecs.
pub fn decode_wic(
    bytes: &[u8],
    limits: &DecodeLimits,
    target_format: ImageFormat,
) -> Result<DecodedImage, ConversionError> {
    limits
        .check_file_size(bytes.len() as u64)
        .map_err(ConversionError::LimitExceeded)?;

    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let factory: IWICImagingFactory =
            CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).map_err(
                |e| ConversionError::DecodeError(format!("WIC CoCreateInstance failed: {e}")),
            )?;

        let stream = factory
            .CreateStream()
            .map_err(|e| ConversionError::DecodeError(format!("WIC CreateStream failed: {e}")))?;

        stream.InitializeFromMemory(bytes).map_err(|e| {
            ConversionError::DecodeError(format!("WIC InitializeFromMemory failed: {e}"))
        })?;

        let decoder = factory
            .CreateDecoderFromStream(&stream, std::ptr::null(), WICDecodeMetadataCacheOnDemand)
            .map_err(|e| {
                // No installed decoder recognizes the data: not a valid file of
                // this format, or its codec is missing (callers check that).
                if e.code() == windows::Win32::Foundation::WINCODEC_ERR_COMPONENTNOTFOUND {
                    ConversionError::CorruptFile(format!(
                        "no Windows decoder recognizes the file ({e})"
                    ))
                } else if let (
                    windows::Win32::Foundation::WINCODEC_ERR_COMPONENTINITIALIZEFAILURE,
                    Some(codec),
                ) = (e.code(), optional_codec(target_format))
                {
                    // The decoder is registered but a codec it needs is missing
                    // (e.g. the HEIF extension's AVIF decoder without AV1).
                    ConversionError::MissingCodec(
                        codec,
                        format!("WIC CreateDecoderFromStream failed: {e}"),
                    )
                } else {
                    ConversionError::DecodeError(format!("WIC CreateDecoderFromStream failed: {e}"))
                }
            })?;

        let frame = decoder
            .GetFrame(0)
            .map_err(|e| ConversionError::DecodeError(format!("WIC GetFrame failed: {e}")))?;

        let mut width = 0;
        let mut height = 0;
        frame
            .GetSize(&mut width, &mut height)
            .map_err(|e| ConversionError::DecodeError(format!("WIC GetSize failed: {e}")))?;

        limits
            .check_dimensions(width, height)
            .map_err(ConversionError::LimitExceeded)?;

        let converter = factory.CreateFormatConverter().map_err(|e| {
            ConversionError::DecodeError(format!("WIC CreateFormatConverter failed: {e}"))
        })?;

        converter
            .Initialize(
                &frame,
                &GUID_WICPixelFormat32bppRGBA,
                WICBitmapDitherTypeNone,
                None,
                0.0,
                WICBitmapPaletteTypeCustom,
            )
            .map_err(|e| {
                ConversionError::DecodeError(format!("WIC FormatConverter Initialize failed: {e}"))
            })?;

        let stride = width * 4;
        let buffer_size = stride as u64 * height as u64;

        limits
            .check_memory_bytes(buffer_size)
            .map_err(ConversionError::LimitExceeded)?;

        let mut rgba_buffer = vec![0u8; buffer_size as usize];
        converter
            .CopyPixels(std::ptr::null(), stride, &mut rgba_buffer)
            .map_err(|e| ConversionError::DecodeError(format!("WIC CopyPixels failed: {e}")))?;

        Ok(DecodedImage::new(
            width,
            height,
            PixelFormat::Rgba8,
            rgba_buffer,
            target_format,
        ))
    }
}
