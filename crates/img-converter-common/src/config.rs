use crate::format::ImageFormat;
use crate::i18n::{Language, LanguageMode};
use crate::policy::{CollisionPolicy, MetadataPolicy, TransparencyPolicy};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub const DEFAULT_CONFIG_INI: &str = r#"# ==============================================================================
# img-converter configuration
# Location: %LOCALAPPDATA%\img-converter\config.ini
# Changes take effect immediately; no restart required.
# ==============================================================================

[menu]
# Comma-separated list of formats shown in the File Explorer right-click submenu.
# Available: avif, bmp, exr, gif, hdr, ico, jpg, pdf, png, tga, tiff, webp
# "all" shows every format (default), e.g. enabled_formats = "jpg, png, webp, pdf"
enabled_formats = "all"

[quality]
# JPEG encoding quality (1 to 100). Default: 100 (maximum quality).
jpeg = 100

# AVIF encoding quality (1 to 100). Default: 100 (maximum quality).
avif = 100

# AVIF encoder speed (1 to 10). 1 = slowest / smallest files, 10 = fastest / larger files.
# Default: 6
avif_speed = 6

# Note: WebP output is always lossless.

[svg]
# Rasterization scale factor for SVG images (0.1 to 10.0).
# 1.0 = native SVG size, 2.0 = 2x resolution (crisp icons/logos). Default: 2.0
scale = 2.0

[ico]
# false = single icon fitted to the image (max 256x256 px).
# true  = multi-resolution bundle with 16, 32, 48, 64, 128 and 256 px layers.
# Default: false
multi_resolution = false

[pdf]
# Rendering resolution for PDF -> image conversion, in dots per inch (72 to 1200).
# Default: 300
dpi = 300

# true  = multi-page PDFs are extracted into "<name>\<name>_page_01.png", ...
# false = pages are saved next to the original PDF file
# Default: true
create_subfolder = true

[transparency]
# Background used when converting transparent images to formats without alpha
# support (JPEG, BMP, HDR, PDF). Options: "white", "black". Default: "white"
matte = "white"

[general]
# true  = overwrite existing files.
# false = keep them and write "photo (1).webp", "photo (2).webp", ... instead.
# Default: false
overwrite = false

# Keep ICC color profiles and EXIF metadata when the output format supports them
# (JPEG, PNG, WebP; ICC only for TIFF). Default: true
preserve_metadata = true

# Context menu language: "auto" (Windows language), "es" (Spanish) or "en" (English)
# Default: "auto"
language = "auto"
"#;

#[derive(Debug, Clone, PartialEq)]
pub struct IniConfig {
    /// Formats offered in the context menu. `None` = all formats.
    pub enabled_formats: Option<Vec<ImageFormat>>,
    pub jpeg_quality: u8,
    pub avif_quality: u8,
    pub avif_speed: u8,
    pub svg_scale: f32,
    pub ico_multi_resolution: bool,
    pub pdf_dpi: u32,
    pub pdf_create_subfolder: bool,
    pub transparency: TransparencyPolicy,
    pub collision: CollisionPolicy,
    pub metadata: MetadataPolicy,
    pub language: LanguageMode,
}

impl Default for IniConfig {
    fn default() -> Self {
        IniConfig {
            enabled_formats: None,
            jpeg_quality: 100,
            avif_quality: 100,
            avif_speed: 6,
            svg_scale: 2.0,
            ico_multi_resolution: false,
            pdf_dpi: 300,
            pdf_create_subfolder: true,
            transparency: TransparencyPolicy::White,
            collision: CollisionPolicy::AutoIncrement,
            metadata: MetadataPolicy::Preserve,
            language: LanguageMode::Auto,
        }
    }
}

/// Parses an INI boolean (`true`/`false`, `yes`/`no`, `on`/`off`, `1`/`0`).
fn parse_bool(val: &str) -> Option<bool> {
    match val.to_ascii_lowercase().as_str() {
        "true" | "yes" | "on" | "1" => Some(true),
        "false" | "no" | "off" | "0" => Some(false),
        _ => None,
    }
}

/// Extracts the value part of an INI line: strips surrounding quotes and any
/// trailing `#` / `;` comment outside of quotes.
fn clean_value(raw: &str) -> &str {
    let raw = raw.trim();
    for quote in ['"', '\''] {
        if let Some(rest) = raw.strip_prefix(quote) {
            return match rest.find(quote) {
                Some(end) => rest[..end].trim(),
                None => rest.trim(),
            };
        }
    }
    match raw.find(['#', ';']) {
        Some(idx) => raw[..idx].trim(),
        None => raw,
    }
}

impl IniConfig {
    pub fn config_path() -> PathBuf {
        if let Ok(local_app_data) = env::var("LOCALAPPDATA") {
            PathBuf::from(local_app_data)
                .join("img-converter")
                .join("config.ini")
        } else if let Ok(user_profile) = env::var("USERPROFILE") {
            PathBuf::from(user_profile)
                .join("AppData")
                .join("Local")
                .join("img-converter")
                .join("config.ini")
        } else {
            PathBuf::from("config.ini")
        }
    }

    /// Loads the user configuration, creating the default file when missing.
    pub fn load_or_default() -> Self {
        let path = Self::config_path();
        if !path.exists() {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(&path, DEFAULT_CONFIG_INI);
            return IniConfig::default();
        }

        Self::load_from_file(&path).unwrap_or_default()
    }

    pub fn load_from_file(path: &Path) -> Option<Self> {
        let content = fs::read_to_string(path).ok()?;
        Some(Self::parse_ini(&content))
    }

    pub fn is_format_enabled(&self, format: ImageFormat) -> bool {
        match &self.enabled_formats {
            None => true,
            Some(list) => list.contains(&format),
        }
    }

    pub fn resolved_language(&self) -> Language {
        Language::resolve(self.language)
    }

    /// Quality setting to use when encoding to `format`.
    pub fn quality_for(&self, format: ImageFormat) -> u8 {
        match format {
            ImageFormat::Jpeg | ImageFormat::Jfif => self.jpeg_quality,
            ImageFormat::Avif => self.avif_quality,
            _ => 100,
        }
    }

    /// Parses INI content. Unknown keys and invalid values are ignored and keep
    /// their defaults, so a partially broken file never disables the extension.
    pub fn parse_ini(content: &str) -> Self {
        let mut config = IniConfig::default();
        let mut current_section = String::new();

        for line in content.lines() {
            let line = line.trim().trim_start_matches('\u{FEFF}');
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }

            if line.starts_with('[') && line.ends_with(']') {
                current_section = line[1..line.len() - 1].trim().to_lowercase();
                continue;
            }

            let Some((key, raw_val)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim().to_lowercase();
            let val = clean_value(raw_val);

            match (current_section.as_str(), key.as_str()) {
                ("menu", "enabled_formats") | ("general", "enabled_formats") => {
                    let clean = val.to_ascii_lowercase();
                    if clean == "all" || clean.is_empty() {
                        config.enabled_formats = None;
                    } else {
                        let list: Vec<ImageFormat> = clean
                            .split(',')
                            .filter_map(|s| ImageFormat::from_str(s.trim()).ok())
                            .collect();
                        if !list.is_empty() {
                            config.enabled_formats = Some(list);
                        }
                    }
                }
                ("quality", "jpeg") | ("", "jpeg") => {
                    if let Ok(q) = val.parse::<u8>() {
                        config.jpeg_quality = q.clamp(1, 100);
                    }
                }
                ("quality", "avif") | ("", "avif") => {
                    if let Ok(q) = val.parse::<u8>() {
                        config.avif_quality = q.clamp(1, 100);
                    }
                }
                ("quality", "avif_speed") | ("", "avif_speed") => {
                    if let Ok(s) = val.parse::<u8>() {
                        config.avif_speed = s.clamp(1, 10);
                    }
                }
                ("svg", "scale") | ("general", "svg_scale") | ("", "svg_scale") => {
                    if let Ok(s) = val.parse::<f32>() {
                        if s.is_finite() {
                            config.svg_scale = s.clamp(0.1, 10.0);
                        }
                    }
                }
                ("ico", "multi_resolution") | ("general", "ico_multi_resolution") => {
                    if let Some(b) = parse_bool(val) {
                        config.ico_multi_resolution = b;
                    }
                }
                ("pdf", "dpi") => {
                    if let Ok(dpi) = val.parse::<u32>() {
                        config.pdf_dpi = dpi.clamp(72, 1200);
                    }
                }
                ("pdf", "create_subfolder") => {
                    if let Some(b) = parse_bool(val) {
                        config.pdf_create_subfolder = b;
                    }
                }
                ("transparency", "matte") | ("", "matte") => {
                    if val.eq_ignore_ascii_case("black") {
                        config.transparency = TransparencyPolicy::Black;
                    } else if val.eq_ignore_ascii_case("white") {
                        config.transparency = TransparencyPolicy::White;
                    }
                }
                ("general", "overwrite") | ("", "overwrite") => {
                    if let Some(b) = parse_bool(val) {
                        config.collision = if b {
                            CollisionPolicy::Overwrite
                        } else {
                            CollisionPolicy::AutoIncrement
                        };
                    }
                }
                ("general", "preserve_metadata") | ("", "preserve_metadata") => {
                    if let Some(b) = parse_bool(val) {
                        config.metadata = if b {
                            MetadataPolicy::Preserve
                        } else {
                            MetadataPolicy::Strip
                        };
                    }
                }
                ("general", "language") | ("", "language") => {
                    config.language = val.parse().unwrap_or_default();
                }
                _ => {}
            }
        }

        config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_default_ini() {
        let config = IniConfig::parse_ini(DEFAULT_CONFIG_INI);
        assert_eq!(config, IniConfig::default());
    }

    #[test]
    fn test_parse_custom_ini() {
        let custom = r#"
        [menu]
        enabled_formats = "jpg, png, webp, pdf"

        [quality]
        jpeg = 85
        avif = 75
        avif_speed = 8

        [svg]
        scale = 2.5

        [ico]
        multi_resolution = true

        [pdf]
        dpi = 600
        create_subfolder = false

        [transparency]
        matte = black

        [general]
        overwrite = true
        preserve_metadata = false
        language = es
        "#;
        let config = IniConfig::parse_ini(custom);
        assert_eq!(
            config.enabled_formats,
            Some(vec![
                ImageFormat::Jpeg,
                ImageFormat::Png,
                ImageFormat::Webp,
                ImageFormat::Pdf
            ])
        );
        assert_eq!(config.jpeg_quality, 85);
        assert_eq!(config.avif_quality, 75);
        assert_eq!(config.avif_speed, 8);
        assert_eq!(config.svg_scale, 2.5);
        assert!(config.ico_multi_resolution);
        assert_eq!(config.pdf_dpi, 600);
        assert!(!config.pdf_create_subfolder);
        assert_eq!(config.transparency, TransparencyPolicy::Black);
        assert_eq!(config.collision, CollisionPolicy::Overwrite);
        assert_eq!(config.metadata, MetadataPolicy::Strip);
        assert_eq!(config.language, LanguageMode::Es);
    }

    #[test]
    fn test_inline_comments_and_quotes() {
        let ini = "[quality]\njpeg = 80 # good enough\n[general]\noverwrite = yes ; sure\nlanguage = 'en' # quoted\n";
        let config = IniConfig::parse_ini(ini);
        assert_eq!(config.jpeg_quality, 80);
        assert_eq!(config.collision, CollisionPolicy::Overwrite);
        assert_eq!(config.language, LanguageMode::En);
    }

    #[test]
    fn test_invalid_values_keep_defaults() {
        let ini = "[quality]\njpeg = abc\navif_speed = 99\n[svg]\nscale = NaN\n[pdf]\ndpi = 5\ncreate_subfolder = maybe\n[transparency]\nmatte = purple\n[menu]\nenabled_formats = xyz, ???\n";
        let config = IniConfig::parse_ini(ini);
        assert_eq!(config.jpeg_quality, 100);
        assert_eq!(config.avif_speed, 10);
        assert_eq!(config.svg_scale, 2.0);
        assert_eq!(config.pdf_dpi, 72);
        assert!(config.pdf_create_subfolder);
        assert_eq!(config.transparency, TransparencyPolicy::White);
        assert_eq!(config.enabled_formats, None);
    }
}
