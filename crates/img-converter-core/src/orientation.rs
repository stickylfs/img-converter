use crate::decoded::DecodedImage;
use std::io::Cursor;

/// Reads the EXIF orientation (1-8) from any container supported by
/// `kamadak-exif` (TIFF-based camera RAWs, JPEG, PNG, WebP, HEIF). Returns 1 when
/// the file has no orientation tag.
pub fn orientation_from_container(bytes: &[u8]) -> u8 {
    exif::Reader::new()
        .read_from_container(&mut Cursor::new(bytes))
        .ok()
        .and_then(|exif| {
            exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)
                .and_then(|field| field.value.get_uint(0))
        })
        .filter(|o| (1..=8).contains(o))
        .map(|o| o as u8)
        .unwrap_or(1)
}

/// Rotates/flips the pixels according to `image.orientation` so the image is
/// upright, then resets the orientation (including inside the preserved EXIF
/// block) to 1 so viewers do not rotate the output a second time.
pub fn apply_exif_orientation(mut image: DecodedImage) -> DecodedImage {
    let orientation = image.orientation;
    image.orientation = 1;
    if let Some(exif) = image.exif_data.as_mut() {
        let _ = image::metadata::Orientation::remove_from_exif_chunk(exif);
    }
    if !(2..=8).contains(&orientation) {
        return image;
    }

    let bpp = image.format.bytes_per_pixel();
    let src_w = image.width as usize;
    let src_h = image.height as usize;
    let swaps_axes = orientation >= 5;
    let (dst_w, dst_h) = if swaps_axes {
        (src_h, src_w)
    } else {
        (src_w, src_h)
    };

    // Maps a source pixel (x, y) to its destination coordinates.
    let map = |x: usize, y: usize| -> (usize, usize) {
        match orientation {
            2 => (src_w - 1 - x, y),             // mirror horizontal
            3 => (src_w - 1 - x, src_h - 1 - y), // rotate 180
            4 => (x, src_h - 1 - y),             // mirror vertical
            5 => (y, x),                         // transpose
            6 => (src_h - 1 - y, x),             // rotate 90 CW
            7 => (src_h - 1 - y, src_w - 1 - x), // transverse
            _ => (y, src_w - 1 - x),             // 8: rotate 270 CW
        }
    };

    let mut out = vec![0u8; image.pixels.len()];
    for y in 0..src_h {
        for x in 0..src_w {
            let (dx, dy) = map(x, y);
            let src = (y * src_w + x) * bpp;
            let dst = (dy * dst_w + dx) * bpp;
            out[dst..dst + bpp].copy_from_slice(&image.pixels[src..src + bpp]);
        }
    }

    image.width = dst_w as u32;
    image.height = dst_h as u32;
    image.pixels = out;
    image
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoded::PixelFormat;
    use img_converter_common::ImageFormat;

    /// 3x2 luma image:
    /// ```text
    /// 1 2 3
    /// 4 5 6
    /// ```
    fn sample(orientation: u8) -> DecodedImage {
        let mut img = DecodedImage::new(
            3,
            2,
            PixelFormat::Luma8,
            vec![1, 2, 3, 4, 5, 6],
            ImageFormat::Jpeg,
        );
        img.orientation = orientation;
        img
    }

    fn apply(orientation: u8) -> (u32, u32, Vec<u8>) {
        let out = apply_exif_orientation(sample(orientation));
        assert_eq!(out.orientation, 1);
        (out.width, out.height, out.pixels)
    }

    #[test]
    fn test_all_orientations() {
        assert_eq!(apply(1), (3, 2, vec![1, 2, 3, 4, 5, 6]));
        assert_eq!(apply(2), (3, 2, vec![3, 2, 1, 6, 5, 4]));
        assert_eq!(apply(3), (3, 2, vec![6, 5, 4, 3, 2, 1]));
        assert_eq!(apply(4), (3, 2, vec![4, 5, 6, 1, 2, 3]));
        assert_eq!(apply(5), (2, 3, vec![1, 4, 2, 5, 3, 6]));
        assert_eq!(apply(6), (2, 3, vec![4, 1, 5, 2, 6, 3]));
        assert_eq!(apply(7), (2, 3, vec![6, 3, 5, 2, 4, 1]));
        assert_eq!(apply(8), (2, 3, vec![3, 6, 2, 5, 1, 4]));
    }

    #[test]
    fn test_invalid_orientation_is_ignored() {
        assert_eq!(apply(0), (3, 2, vec![1, 2, 3, 4, 5, 6]));
        assert_eq!(apply(9), (3, 2, vec![1, 2, 3, 4, 5, 6]));
    }
}
