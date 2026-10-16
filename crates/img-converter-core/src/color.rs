//! sRGB transfer function helpers for converting between 8-bit sRGB and linear
//! floating point (HDR / OpenEXR) images.

use std::sync::OnceLock;

/// Converts a linear light value to an 8-bit sRGB value (clipping highlights above 1.0).
pub fn linear_to_srgb_u8(linear: f32) -> u8 {
    let v = if linear.is_nan() {
        0.0
    } else {
        linear.clamp(0.0, 1.0)
    };
    let encoded = if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0).round() as u8
}

/// Converts an 8-bit sRGB value to linear light.
pub fn srgb_u8_to_linear(value: u8) -> f32 {
    static LUT: OnceLock<[f32; 256]> = OnceLock::new();
    LUT.get_or_init(|| {
        let mut table = [0.0f32; 256];
        for (i, slot) in table.iter_mut().enumerate() {
            let v = i as f32 / 255.0;
            *slot = if v <= 0.040_45 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            };
        }
        table
    })[value as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip_all_values() {
        for v in 0..=255u8 {
            assert_eq!(linear_to_srgb_u8(srgb_u8_to_linear(v)), v);
        }
    }

    #[test]
    fn test_out_of_range_values() {
        assert_eq!(linear_to_srgb_u8(-1.0), 0);
        assert_eq!(linear_to_srgb_u8(8.0), 255);
        assert_eq!(linear_to_srgb_u8(f32::NAN), 0);
        // Linear mid-grey (18%) is roughly sRGB 118.
        assert!((115..=121).contains(&linear_to_srgb_u8(0.18)));
    }
}
