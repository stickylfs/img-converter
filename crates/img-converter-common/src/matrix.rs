use crate::config::IniConfig;
use crate::format::ImageFormat;

/// Centralized conversion matrix defining allowed source -> target format transitions.
pub struct ConversionMatrix;

impl ConversionMatrix {
    /// Returns the slice of allowed target formats for a given single source format (in alphabetical order).
    /// Crucial rule: Never include the source format itself.
    pub fn valid_targets(source: ImageFormat) -> &'static [ImageFormat] {
        match source {
            ImageFormat::Avif => &[
                ImageFormat::Bmp,
                ImageFormat::Exr,
                ImageFormat::Gif,
                ImageFormat::Hdr,
                ImageFormat::Ico,
                ImageFormat::Jpeg,
                ImageFormat::Pdf,
                ImageFormat::Png,
                ImageFormat::Tga,
                ImageFormat::Tiff,
                ImageFormat::Webp,
            ],
            ImageFormat::Bmp => &[
                ImageFormat::Avif,
                ImageFormat::Exr,
                ImageFormat::Gif,
                ImageFormat::Hdr,
                ImageFormat::Ico,
                ImageFormat::Jpeg,
                ImageFormat::Pdf,
                ImageFormat::Png,
                ImageFormat::Tga,
                ImageFormat::Tiff,
                ImageFormat::Webp,
            ],
            ImageFormat::Dds => ImageFormat::OUTPUT_FORMATS,
            ImageFormat::Exr => &[
                ImageFormat::Avif,
                ImageFormat::Bmp,
                ImageFormat::Gif,
                ImageFormat::Hdr,
                ImageFormat::Ico,
                ImageFormat::Jpeg,
                ImageFormat::Pdf,
                ImageFormat::Png,
                ImageFormat::Tga,
                ImageFormat::Tiff,
                ImageFormat::Webp,
            ],
            ImageFormat::Gif => &[
                ImageFormat::Avif,
                ImageFormat::Bmp,
                ImageFormat::Exr,
                ImageFormat::Hdr,
                ImageFormat::Ico,
                ImageFormat::Jpeg,
                ImageFormat::Pdf,
                ImageFormat::Png,
                ImageFormat::Tga,
                ImageFormat::Tiff,
                ImageFormat::Webp,
            ],
            ImageFormat::Hdr => &[
                ImageFormat::Avif,
                ImageFormat::Bmp,
                ImageFormat::Exr,
                ImageFormat::Gif,
                ImageFormat::Ico,
                ImageFormat::Jpeg,
                ImageFormat::Pdf,
                ImageFormat::Png,
                ImageFormat::Tga,
                ImageFormat::Tiff,
                ImageFormat::Webp,
            ],
            ImageFormat::Heic => ImageFormat::OUTPUT_FORMATS,
            ImageFormat::Ico => &[
                ImageFormat::Avif,
                ImageFormat::Bmp,
                ImageFormat::Exr,
                ImageFormat::Gif,
                ImageFormat::Hdr,
                ImageFormat::Jpeg,
                ImageFormat::Pdf,
                ImageFormat::Png,
                ImageFormat::Tga,
                ImageFormat::Tiff,
                ImageFormat::Webp,
            ],
            ImageFormat::Jfif => ImageFormat::OUTPUT_FORMATS,
            ImageFormat::Jpeg => &[
                ImageFormat::Avif,
                ImageFormat::Bmp,
                ImageFormat::Exr,
                ImageFormat::Gif,
                ImageFormat::Hdr,
                ImageFormat::Ico,
                ImageFormat::Pdf,
                ImageFormat::Png,
                ImageFormat::Tga,
                ImageFormat::Tiff,
                ImageFormat::Webp,
            ],
            ImageFormat::Pdf => &[
                ImageFormat::Avif,
                ImageFormat::Bmp,
                ImageFormat::Exr,
                ImageFormat::Gif,
                ImageFormat::Hdr,
                ImageFormat::Ico,
                ImageFormat::Jpeg,
                ImageFormat::Png,
                ImageFormat::Tga,
                ImageFormat::Tiff,
                ImageFormat::Webp,
            ],
            ImageFormat::Png => &[
                ImageFormat::Avif,
                ImageFormat::Bmp,
                ImageFormat::Exr,
                ImageFormat::Gif,
                ImageFormat::Hdr,
                ImageFormat::Ico,
                ImageFormat::Jpeg,
                ImageFormat::Pdf,
                ImageFormat::Tga,
                ImageFormat::Tiff,
                ImageFormat::Webp,
            ],
            ImageFormat::Raw => ImageFormat::OUTPUT_FORMATS,
            ImageFormat::Svg => ImageFormat::OUTPUT_FORMATS,
            ImageFormat::Tga => &[
                ImageFormat::Avif,
                ImageFormat::Bmp,
                ImageFormat::Exr,
                ImageFormat::Gif,
                ImageFormat::Hdr,
                ImageFormat::Ico,
                ImageFormat::Jpeg,
                ImageFormat::Pdf,
                ImageFormat::Png,
                ImageFormat::Tiff,
                ImageFormat::Webp,
            ],
            ImageFormat::Tiff => &[
                ImageFormat::Avif,
                ImageFormat::Bmp,
                ImageFormat::Exr,
                ImageFormat::Gif,
                ImageFormat::Hdr,
                ImageFormat::Ico,
                ImageFormat::Jpeg,
                ImageFormat::Pdf,
                ImageFormat::Png,
                ImageFormat::Tga,
                ImageFormat::Webp,
            ],
            ImageFormat::Webp => &[
                ImageFormat::Avif,
                ImageFormat::Bmp,
                ImageFormat::Exr,
                ImageFormat::Gif,
                ImageFormat::Hdr,
                ImageFormat::Ico,
                ImageFormat::Jpeg,
                ImageFormat::Pdf,
                ImageFormat::Png,
                ImageFormat::Tga,
                ImageFormat::Tiff,
            ],
        }
    }

    /// Checks if a single conversion from `source` to `target` is permitted.
    pub fn is_conversion_allowed(source: ImageFormat, target: ImageFormat) -> bool {
        Self::valid_targets(source).contains(&target)
    }

    /// Computes the valid target formats for a collection of input formats.
    pub fn get_targets_for_selection(sources: &[ImageFormat]) -> Vec<ImageFormat> {
        if sources.is_empty() {
            return Vec::new();
        }

        let first = sources[0];
        let all_same = sources.iter().all(|&f| f == first);

        if all_same {
            Self::valid_targets(first).to_vec()
        } else {
            ImageFormat::OUTPUT_FORMATS.to_vec()
        }
    }

    /// Computes valid targets for a selection filtered through the user's config whitelist.
    pub fn get_targets_for_selection_with_config(
        sources: &[ImageFormat],
        config: &IniConfig,
    ) -> Vec<ImageFormat> {
        Self::get_targets_for_selection(sources)
            .into_iter()
            .filter(|&fmt| config.is_format_enabled(fmt))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_format_targets() {
        let targets = ConversionMatrix::valid_targets(ImageFormat::Jpeg);
        assert!(!targets.contains(&ImageFormat::Jpeg));
        assert!(targets.contains(&ImageFormat::Png));
        assert!(targets.contains(&ImageFormat::Pdf));
        assert!(targets.contains(&ImageFormat::Bmp));
        assert!(targets.contains(&ImageFormat::Tiff));
    }

    #[test]
    fn test_no_self_conversion() {
        for &fmt in ImageFormat::OUTPUT_FORMATS {
            assert!(!ConversionMatrix::is_conversion_allowed(fmt, fmt));
        }
    }

    #[test]
    fn test_whitelist_filtering() {
        let config = IniConfig {
            enabled_formats: Some(vec![ImageFormat::Png, ImageFormat::Webp]),
            ..Default::default()
        };

        let targets =
            ConversionMatrix::get_targets_for_selection_with_config(&[ImageFormat::Jpeg], &config);
        assert_eq!(targets, vec![ImageFormat::Png, ImageFormat::Webp]);
    }
}
