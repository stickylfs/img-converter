use crate::decoded::{DecodedImage, PixelFormat};
use img_converter_common::TransparencyPolicy;

/// Composites an RGBA image onto a solid background matte according to the transparency policy.
pub fn composite_transparency(image: &DecodedImage, policy: TransparencyPolicy) -> DecodedImage {
    if !image.has_alpha() {
        return image.clone();
    }

    let matte = policy.matte_color();
    let [mr, mg, mb] = [matte[0] as u32, matte[1] as u32, matte[2] as u32];

    let rgba = image.to_rgba8();
    let pixel_count = (image.width as usize) * (image.height as usize);
    let mut out_rgb = Vec::with_capacity(pixel_count * 3);

    for chunk in rgba.chunks_exact(4) {
        let r = chunk[0] as u32;
        let g = chunk[1] as u32;
        let b = chunk[2] as u32;
        let a = chunk[3] as u32;

        if a == 255 {
            out_rgb.push(chunk[0]);
            out_rgb.push(chunk[1]);
            out_rgb.push(chunk[2]);
        } else if a == 0 {
            out_rgb.push(matte[0]);
            out_rgb.push(matte[1]);
            out_rgb.push(matte[2]);
        } else {
            let inv_a = 255 - a;
            let out_r = ((r * a + mr * inv_a + 127) / 255) as u8;
            let out_g = ((g * a + mg * inv_a + 127) / 255) as u8;
            let out_b = ((b * a + mb * inv_a + 127) / 255) as u8;
            out_rgb.push(out_r);
            out_rgb.push(out_g);
            out_rgb.push(out_b);
        }
    }

    let mut result = DecodedImage::new(
        image.width,
        image.height,
        PixelFormat::Rgb8,
        out_rgb,
        image.source_format,
    );
    result.icc_profile = image.icc_profile.clone();
    result.exif_data = image.exif_data.clone();
    result.orientation = image.orientation;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use img_converter_common::ImageFormat;

    #[test]
    fn test_alpha_compositing_white() {
        // Semi-transparent red: (255, 0, 0, 128) over White (255, 255, 255)
        // r: (255*128 + 255*127 + 127)/255 = 255
        // g: (0*128 + 255*127 + 127)/255 = 127
        // b: (0*128 + 255*127 + 127)/255 = 127
        let img = DecodedImage::new(
            1,
            1,
            PixelFormat::Rgba8,
            vec![255, 0, 0, 128],
            ImageFormat::Png,
        );
        let composited = composite_transparency(&img, TransparencyPolicy::White);
        assert_eq!(composited.pixels, vec![255, 127, 127]);
    }

    #[test]
    fn test_fully_transparent_to_white() {
        let img = DecodedImage::new(1, 1, PixelFormat::Rgba8, vec![0, 0, 0, 0], ImageFormat::Png);
        let composited = composite_transparency(&img, TransparencyPolicy::White);
        assert_eq!(composited.pixels, vec![255, 255, 255]);
    }
}
