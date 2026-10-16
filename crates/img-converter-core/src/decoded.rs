use crate::color::linear_to_srgb_u8;
use image::DynamicImage;
use img_converter_common::ImageFormat;

/// Internal pixel color representation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Rgb8,
    Rgba8,
    Luma8,
    LumaA8,
}

impl PixelFormat {
    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            PixelFormat::Rgb8 => 3,
            PixelFormat::Rgba8 => 4,
            PixelFormat::Luma8 => 1,
            PixelFormat::LumaA8 => 2,
        }
    }

    pub fn has_alpha(&self) -> bool {
        match self {
            PixelFormat::Rgba8 | PixelFormat::LumaA8 => true,
            PixelFormat::Rgb8 | PixelFormat::Luma8 => false,
        }
    }
}

/// Normalized in-memory decoded image (8-bit sRGB) with its metadata.
#[derive(Debug, Clone)]
pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub pixels: Vec<u8>,
    pub source_format: ImageFormat,
    /// Embedded ICC color profile, if any.
    pub icc_profile: Option<Vec<u8>>,
    /// Raw EXIF block (TIFF structure, without the `Exif\0\0` prefix), if any.
    pub exif_data: Option<Vec<u8>>,
    /// EXIF orientation (1-8) that still has to be applied to `pixels`.
    pub orientation: u8,
}

impl DecodedImage {
    pub fn new(
        width: u32,
        height: u32,
        format: PixelFormat,
        pixels: Vec<u8>,
        source_format: ImageFormat,
    ) -> Self {
        Self {
            width,
            height,
            format,
            pixels,
            source_format,
            icc_profile: None,
            exif_data: None,
            orientation: 1,
        }
    }

    /// Converts a decoded `image` crate buffer into the internal 8-bit representation.
    ///
    /// High bit depth images are scaled down to 8 bits; floating point images
    /// (HDR, EXR) hold linear light and are converted to sRGB.
    pub fn from_dynamic(img: DynamicImage, source_format: ImageFormat) -> Self {
        let (width, height) = (img.width(), img.height());
        let (format, pixels) = match img {
            DynamicImage::ImageRgb8(i) => (PixelFormat::Rgb8, i.into_raw()),
            DynamicImage::ImageRgba8(i) => (PixelFormat::Rgba8, i.into_raw()),
            DynamicImage::ImageLuma8(i) => (PixelFormat::Luma8, i.into_raw()),
            DynamicImage::ImageLumaA8(i) => (PixelFormat::LumaA8, i.into_raw()),
            DynamicImage::ImageLuma16(_) => (PixelFormat::Luma8, img.to_luma8().into_raw()),
            DynamicImage::ImageLumaA16(_) => (PixelFormat::LumaA8, img.to_luma_alpha8().into_raw()),
            DynamicImage::ImageRgb32F(i) => (
                PixelFormat::Rgb8,
                i.into_raw().into_iter().map(linear_to_srgb_u8).collect(),
            ),
            DynamicImage::ImageRgba32F(i) => {
                let mut out = Vec::with_capacity(i.as_raw().len());
                for px in i.into_raw().chunks_exact(4) {
                    out.push(linear_to_srgb_u8(px[0]));
                    out.push(linear_to_srgb_u8(px[1]));
                    out.push(linear_to_srgb_u8(px[2]));
                    // Alpha is not gamma encoded.
                    out.push((px[3].clamp(0.0, 1.0) * 255.0).round() as u8);
                }
                (PixelFormat::Rgba8, out)
            }
            other if other.color().has_alpha() => (PixelFormat::Rgba8, other.to_rgba8().into_raw()),
            other => (PixelFormat::Rgb8, other.to_rgb8().into_raw()),
        };
        Self::new(width, height, format, pixels, source_format)
    }

    pub fn has_alpha(&self) -> bool {
        self.format.has_alpha()
    }

    /// Converts image pixels to standard RGBA8 format.
    pub fn to_rgba8(&self) -> Vec<u8> {
        match self.format {
            PixelFormat::Rgba8 => self.pixels.clone(),
            PixelFormat::Rgb8 => self
                .pixels
                .chunks_exact(3)
                .flat_map(|c| [c[0], c[1], c[2], 255])
                .collect(),
            PixelFormat::Luma8 => self.pixels.iter().flat_map(|&y| [y, y, y, 255]).collect(),
            PixelFormat::LumaA8 => self
                .pixels
                .chunks_exact(2)
                .flat_map(|c| [c[0], c[0], c[0], c[1]])
                .collect(),
        }
    }

    /// Converts image pixels to RGB8, discarding alpha if present (without compositing).
    pub fn to_rgb8_raw(&self) -> Vec<u8> {
        match self.format {
            PixelFormat::Rgb8 => self.pixels.clone(),
            PixelFormat::Rgba8 => self
                .pixels
                .chunks_exact(4)
                .flat_map(|c| [c[0], c[1], c[2]])
                .collect(),
            PixelFormat::Luma8 => self.pixels.iter().flat_map(|&y| [y, y, y]).collect(),
            PixelFormat::LumaA8 => self
                .pixels
                .chunks_exact(2)
                .flat_map(|c| [c[0], c[0], c[0]])
                .collect(),
        }
    }

    /// Drops the alpha channel when every pixel is fully opaque (common for WIC,
    /// DDS and AVIF decoders that always return RGBA), so opaque sources do not
    /// produce needlessly larger outputs or trigger matte compositing.
    pub fn drop_opaque_alpha(&mut self) {
        let opaque = match self.format {
            PixelFormat::Rgba8 => self.pixels.chunks_exact(4).all(|c| c[3] == 255),
            PixelFormat::LumaA8 => self.pixels.chunks_exact(2).all(|c| c[1] == 255),
            PixelFormat::Rgb8 | PixelFormat::Luma8 => return,
        };
        if opaque {
            self.pixels = match self.format {
                PixelFormat::LumaA8 => self.pixels.chunks_exact(2).map(|c| c[0]).collect(),
                _ => self.to_rgb8_raw(),
            };
            self.format = match self.format {
                PixelFormat::LumaA8 => PixelFormat::Luma8,
                _ => PixelFormat::Rgb8,
            };
        }
    }

    /// Returns an RGBA8 `image` crate buffer (used for resizing and GIF/ICO encoding).
    pub fn to_rgba_image(&self) -> Option<image::RgbaImage> {
        image::RgbaImage::from_raw(self.width, self.height, self.to_rgba8())
    }
}
