use crate::codecs::{decode_with_image_crate, EncodeOptions};
use crate::codecs::{ImageDecoder, ImageEncoder};
use crate::decoded::DecodedImage;
use image::imageops::FilterType;
use image::{DynamicImage, ImageFormat as ImgFmt, RgbaImage};
use img_converter_common::{ConversionError, DecodeLimits, ImageFormat};
use std::io::Cursor;

/// Layer sizes written by the multi-resolution ICO encoder.
const MULTI_RESOLUTION_SIZES: &[u32] = &[16, 32, 48, 64, 128, 256];

pub struct IcoCodec;

impl ImageDecoder for IcoCodec {
    /// Decodes the largest image stored in the icon.
    fn decode(bytes: &[u8], limits: &DecodeLimits) -> Result<DecodedImage, ConversionError> {
        decode_with_image_crate(bytes, ImgFmt::Ico, ImageFormat::Ico, limits)
    }
}

impl ImageEncoder for IcoCodec {
    fn encode(image: &DecodedImage, options: &EncodeOptions) -> Result<Vec<u8>, ConversionError> {
        let rgba = image.to_rgba_image().ok_or_else(|| {
            ConversionError::EncodeError("Invalid RGBA buffer for ICO encoding".to_string())
        })?;

        if options.ico_multi_resolution {
            encode_multi_resolution(&pad_to_square(rgba))
        } else {
            encode_single(DynamicImage::ImageRgba8(rgba))
        }
    }
}

fn encode_err(e: impl std::fmt::Display) -> ConversionError {
    ConversionError::EncodeError(e.to_string())
}

/// Single icon fitted within 256x256 px, keeping the aspect ratio.
fn encode_single(image: DynamicImage) -> Result<Vec<u8>, ConversionError> {
    let fitted = if image.width() > 256 || image.height() > 256 {
        image.resize(256, 256, FilterType::Lanczos3)
    } else {
        image
    };

    let mut buffer = Vec::new();
    fitted
        .write_to(&mut Cursor::new(&mut buffer), ImgFmt::Ico)
        .map_err(encode_err)?;
    Ok(buffer)
}

/// Centers a non-square image on a transparent square canvas so icon layers are
/// not stretched.
fn pad_to_square(image: RgbaImage) -> RgbaImage {
    let (w, h) = image.dimensions();
    if w == h {
        return image;
    }
    let side = w.max(h);
    let mut canvas = RgbaImage::new(side, side);
    image::imageops::overlay(
        &mut canvas,
        &image,
        ((side - w) / 2) as i64,
        ((side - h) / 2) as i64,
    );
    canvas
}

/// Multi-resolution icon with one PNG-compressed layer per size.
fn encode_multi_resolution(square: &RgbaImage) -> Result<Vec<u8>, ConversionError> {
    let source = DynamicImage::ImageRgba8(square.clone());
    let mut frames: Vec<(u32, Vec<u8>)> = Vec::with_capacity(MULTI_RESOLUTION_SIZES.len());

    for &size in MULTI_RESOLUTION_SIZES {
        let resized = source.resize_exact(size, size, FilterType::Lanczos3);
        let mut png = Vec::new();
        resized
            .write_to(&mut Cursor::new(&mut png), ImgFmt::Png)
            .map_err(encode_err)?;
        frames.push((size, png));
    }

    // ICONDIR header: reserved (2), type = 1 (2), image count (2)
    let mut out: Vec<u8> = vec![0x00, 0x00, 0x01, 0x00];
    out.extend_from_slice(&(frames.len() as u16).to_le_bytes());

    // ICONDIRENTRY records (16 bytes each), followed by the PNG payloads
    let mut offset = (6 + frames.len() * 16) as u32;
    for (size, png) in &frames {
        let dim = if *size >= 256 { 0 } else { *size as u8 }; // 0 means 256 px
        out.extend_from_slice(&[dim, dim, 0, 0]); // width, height, palette, reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // color planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&(png.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for (_, png) in &frames {
        out.extend_from_slice(png);
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoded::PixelFormat;

    #[test]
    fn test_multi_resolution_keeps_aspect_ratio() {
        // 4x2 image: left half red, right half blue.
        let mut pixels = Vec::new();
        for _ in 0..2 {
            pixels.extend_from_slice(&[255, 0, 0, 255, 255, 0, 0, 255]);
            pixels.extend_from_slice(&[0, 0, 255, 255, 0, 0, 255, 255]);
        }
        let image = DecodedImage::new(4, 2, PixelFormat::Rgba8, pixels, ImageFormat::Png);
        let options = EncodeOptions {
            ico_multi_resolution: true,
            ..Default::default()
        };

        let ico = IcoCodec::encode(&image, &options).unwrap();
        assert_eq!(u16::from_le_bytes([ico[4], ico[5]]), 6);

        let decoded = IcoCodec::decode(&ico, &DecodeLimits::default()).unwrap();
        assert_eq!((decoded.width, decoded.height), (256, 256));
        // The padding rows above the content must stay transparent.
        assert_eq!(decoded.to_rgba8()[3], 0);
    }
}
