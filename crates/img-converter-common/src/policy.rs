use serde::{Deserialize, Serialize};

/// Policy for handling transparency (alpha channel) when converting
/// an image with transparency (e.g. PNG, WebP) to a format without
/// native alpha support (e.g. JPEG).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TransparencyPolicy {
    /// Composite over solid white (#FFFFFF). Default policy.
    #[default]
    White,
    /// Composite over solid black (#000000).
    Black,
}

impl TransparencyPolicy {
    /// Returns the background RGB color array [R, G, B] for this policy.
    #[inline]
    pub const fn matte_color(&self) -> [u8; 3] {
        match *self {
            TransparencyPolicy::White => [255, 255, 255],
            TransparencyPolicy::Black => [0, 0, 0],
        }
    }
}

/// Policy for resolving filename collisions when the target file already exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum CollisionPolicy {
    /// If photo.webp exists, write to photo (1).webp, photo (2).webp, etc.
    #[default]
    AutoIncrement,
    /// Overwrite the target file if it already exists.
    Overwrite,
    /// Fail the conversion if the target file already exists.
    Fail,
}

/// Policy for preserving metadata across conversions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MetadataPolicy {
    /// Preserve EXIF and ICC profiles when the target format and codec support it.
    #[default]
    Preserve,
    /// Strip all metadata for smaller output file size.
    Strip,
}
