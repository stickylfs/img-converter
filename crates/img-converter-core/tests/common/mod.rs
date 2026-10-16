#![allow(dead_code)]

use image::{ImageBuffer, Rgb, Rgba};
use img_converter_common::{ConversionRequest, ImageFormat, IniConfig};
use std::fs;
use std::path::{Path, PathBuf};

/// Creates an empty, test-specific temporary directory.
pub fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "img_converter_test_{}_{}",
        name,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Default request (as produced by a default `config.ini`) for a set of files.
pub fn request(files: Vec<PathBuf>, target: ImageFormat) -> ConversionRequest {
    ConversionRequest::from_config(files, target, &IniConfig::default())
}

/// Writes an opaque RGB gradient JPEG.
pub fn write_jpeg(path: &Path, width: u32, height: u32) {
    let img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::from_fn(width, height, |x, y| {
        Rgb([(x * 7) as u8, (y * 7) as u8, 180])
    });
    img.save(path).unwrap();
}

/// Writes an RGBA PNG whose right half is semi-transparent.
pub fn write_transparent_png(path: &Path, width: u32, height: u32) {
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_fn(width, height, |x, y| {
        let alpha = if x < width / 2 { 255 } else { 100 };
        Rgba([(x * 7) as u8, (y * 7) as u8, 200, alpha])
    });
    img.save(path).unwrap();
}

pub fn write_solid_png(path: &Path, width: u32, height: u32, color: [u8; 4]) {
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::from_pixel(width, height, Rgba(color));
    img.save(path).unwrap();
}

pub fn write_svg(path: &Path) {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32" viewBox="0 0 32 32">
        <circle cx="16" cy="16" r="14" fill="#3498db" />
        <rect x="10" y="10" width="12" height="12" fill="#e74c3c" opacity="0.8"/>
    </svg>"##;
    fs::write(path, svg).unwrap();
}

/// Minimal little-endian EXIF block containing only an orientation tag.
pub fn exif_with_orientation(orientation: u16) -> Vec<u8> {
    let mut exif = b"II*\0".to_vec();
    exif.extend_from_slice(&8u32.to_le_bytes()); // offset of IFD0
    exif.extend_from_slice(&1u16.to_le_bytes()); // one entry
    exif.extend_from_slice(&0x0112u16.to_le_bytes()); // Orientation
    exif.extend_from_slice(&3u16.to_le_bytes()); // SHORT
    exif.extend_from_slice(&1u32.to_le_bytes()); // count
    exif.extend_from_slice(&orientation.to_le_bytes());
    exif.extend_from_slice(&[0, 0]); // value padding
    exif.extend_from_slice(&0u32.to_le_bytes()); // no next IFD
    exif
}
