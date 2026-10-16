use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::Path;
use std::str::FromStr;

/// Supported image formats for the converter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    Avif,
    Bmp,
    Dds,
    Exr,
    Gif,
    Hdr,
    Heic,
    Ico,
    Jfif,
    Jpeg,
    Pdf,
    Png,
    Raw,
    Svg,
    Tga,
    Tiff,
    Webp,
}

impl ImageFormat {
    /// List of all supported input formats.
    pub const INPUT_FORMATS: &'static [ImageFormat] = &[
        ImageFormat::Avif,
        ImageFormat::Bmp,
        ImageFormat::Dds,
        ImageFormat::Exr,
        ImageFormat::Gif,
        ImageFormat::Hdr,
        ImageFormat::Heic,
        ImageFormat::Ico,
        ImageFormat::Jfif,
        ImageFormat::Jpeg,
        ImageFormat::Pdf,
        ImageFormat::Png,
        ImageFormat::Raw,
        ImageFormat::Svg,
        ImageFormat::Tga,
        ImageFormat::Tiff,
        ImageFormat::Webp,
    ];

    /// List of all supported output formats in alphabetical order.
    pub const OUTPUT_FORMATS: &'static [ImageFormat] = &[
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
        ImageFormat::Webp,
    ];

    /// Primary file extension (without dot).
    pub fn extension(&self) -> &'static str {
        match self {
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Jfif => "jfif",
            ImageFormat::Png => "png",
            ImageFormat::Webp => "webp",
            ImageFormat::Avif => "avif",
            ImageFormat::Svg => "svg",
            ImageFormat::Gif => "gif",
            ImageFormat::Ico => "ico",
            ImageFormat::Bmp => "bmp",
            ImageFormat::Tiff => "tiff",
            ImageFormat::Tga => "tga",
            ImageFormat::Dds => "dds",
            ImageFormat::Hdr => "hdr",
            ImageFormat::Exr => "exr",
            ImageFormat::Heic => "heic",
            ImageFormat::Raw => "dng",
            ImageFormat::Pdf => "pdf",
        }
    }

    /// Display label for context menu and UI.
    pub fn display_name(&self) -> &'static str {
        match self {
            ImageFormat::Jpeg => "JPG",
            ImageFormat::Jfif => "JFIF",
            ImageFormat::Png => "PNG",
            ImageFormat::Webp => "WEBP",
            ImageFormat::Avif => "AVIF",
            ImageFormat::Svg => "SVG",
            ImageFormat::Gif => "GIF",
            ImageFormat::Ico => "ICO",
            ImageFormat::Bmp => "BMP",
            ImageFormat::Tiff => "TIFF",
            ImageFormat::Tga => "TGA",
            ImageFormat::Dds => "DDS",
            ImageFormat::Hdr => "HDR",
            ImageFormat::Exr => "EXR",
            ImageFormat::Heic => "HEIC",
            ImageFormat::Raw => "RAW",
            ImageFormat::Pdf => "PDF",
        }
    }

    /// Returns standard MIME type string.
    pub fn mime_type(&self) -> &'static str {
        match self {
            ImageFormat::Jpeg | ImageFormat::Jfif => "image/jpeg",
            ImageFormat::Png => "image/png",
            ImageFormat::Webp => "image/webp",
            ImageFormat::Avif => "image/avif",
            ImageFormat::Svg => "image/svg+xml",
            ImageFormat::Gif => "image/gif",
            ImageFormat::Ico => "image/x-icon",
            ImageFormat::Bmp => "image/bmp",
            ImageFormat::Tiff => "image/tiff",
            ImageFormat::Tga => "image/x-targa",
            ImageFormat::Dds => "image/vnd-ms.dds",
            ImageFormat::Hdr => "image/vnd.radiance",
            ImageFormat::Exr => "image/x-exr",
            ImageFormat::Heic => "image/heic",
            ImageFormat::Raw => "image/x-dcraw",
            ImageFormat::Pdf => "application/pdf",
        }
    }

    /// Whether this format supports an alpha (transparency) channel.
    pub fn supports_alpha(&self) -> bool {
        match self {
            ImageFormat::Jpeg
            | ImageFormat::Jfif
            | ImageFormat::Bmp
            | ImageFormat::Hdr
            | ImageFormat::Raw
            | ImageFormat::Pdf => false,
            ImageFormat::Png
            | ImageFormat::Webp
            | ImageFormat::Avif
            | ImageFormat::Svg
            | ImageFormat::Gif
            | ImageFormat::Ico
            | ImageFormat::Tiff
            | ImageFormat::Tga
            | ImageFormat::Dds
            | ImageFormat::Exr
            | ImageFormat::Heic => true,
        }
    }

    /// Fast detection of image format purely based on file extension.
    pub fn from_extension(ext: &str) -> Option<ImageFormat> {
        let clean = ext.trim_start_matches('.').trim();
        if clean.eq_ignore_ascii_case("jpg") || clean.eq_ignore_ascii_case("jpeg") {
            Some(ImageFormat::Jpeg)
        } else if clean.eq_ignore_ascii_case("jfif") {
            Some(ImageFormat::Jfif)
        } else if clean.eq_ignore_ascii_case("png") {
            Some(ImageFormat::Png)
        } else if clean.eq_ignore_ascii_case("webp") {
            Some(ImageFormat::Webp)
        } else if clean.eq_ignore_ascii_case("avif") {
            Some(ImageFormat::Avif)
        } else if clean.eq_ignore_ascii_case("svg") {
            Some(ImageFormat::Svg)
        } else if clean.eq_ignore_ascii_case("gif") {
            Some(ImageFormat::Gif)
        } else if clean.eq_ignore_ascii_case("ico") {
            Some(ImageFormat::Ico)
        } else if clean.eq_ignore_ascii_case("bmp") {
            Some(ImageFormat::Bmp)
        } else if clean.eq_ignore_ascii_case("tiff") || clean.eq_ignore_ascii_case("tif") {
            Some(ImageFormat::Tiff)
        } else if clean.eq_ignore_ascii_case("tga") {
            Some(ImageFormat::Tga)
        } else if clean.eq_ignore_ascii_case("dds") {
            Some(ImageFormat::Dds)
        } else if clean.eq_ignore_ascii_case("hdr") {
            Some(ImageFormat::Hdr)
        } else if clean.eq_ignore_ascii_case("exr") {
            Some(ImageFormat::Exr)
        } else if clean.eq_ignore_ascii_case("heic") || clean.eq_ignore_ascii_case("heif") {
            Some(ImageFormat::Heic)
        } else if clean.eq_ignore_ascii_case("cr2")
            || clean.eq_ignore_ascii_case("cr3")
            || clean.eq_ignore_ascii_case("nef")
            || clean.eq_ignore_ascii_case("arw")
            || clean.eq_ignore_ascii_case("dng")
            || clean.eq_ignore_ascii_case("raw")
        {
            Some(ImageFormat::Raw)
        } else if clean.eq_ignore_ascii_case("pdf") {
            Some(ImageFormat::Pdf)
        } else {
            None
        }
    }

    /// Fast detection of image format from file path extension.
    pub fn from_path(path: &Path) -> Option<ImageFormat> {
        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(Self::from_extension)
    }

    /// Magic byte sniffing for reliable format detection from binary header.
    ///
    /// JFIF files are reported as [`ImageFormat::Jpeg`]: almost every JPEG carries a
    /// JFIF segment, so `.jfif` is only distinguished by its file extension.
    pub fn detect_from_bytes(bytes: &[u8]) -> Option<ImageFormat> {
        if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return Some(ImageFormat::Jpeg);
        }
        if bytes.starts_with(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
            return Some(ImageFormat::Png);
        }
        // RIFF .... WEBP
        if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
            return Some(ImageFormat::Webp);
        }
        if bytes.starts_with(b"%PDF-") {
            return Some(ImageFormat::Pdf);
        }
        if bytes.starts_with(b"BM") {
            return Some(ImageFormat::Bmp);
        }
        // TIFF II*\0 or MM\0* (Canon CR2 stores "CR" right after the TIFF header)
        if bytes.starts_with(&[0x49, 0x49, 0x2A, 0x00]) {
            if bytes.len() >= 10 && &bytes[8..10] == b"CR" {
                return Some(ImageFormat::Raw);
            }
            return Some(ImageFormat::Tiff);
        }
        if bytes.starts_with(&[0x4D, 0x4D, 0x00, 0x2A]) {
            return Some(ImageFormat::Tiff);
        }
        if bytes.starts_with(b"DDS ") {
            return Some(ImageFormat::Dds);
        }
        if bytes.starts_with(&[0x76, 0x2F, 0x31, 0x01]) {
            return Some(ImageFormat::Exr);
        }
        if bytes.starts_with(b"#?RADIANCE\n") || bytes.starts_with(b"#?RGBE\n") {
            return Some(ImageFormat::Hdr);
        }
        if let Some(format) = Self::detect_isobmff(bytes) {
            return Some(format);
        }
        if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
            return Some(ImageFormat::Gif);
        }
        if bytes.starts_with(&[0x00, 0x00, 0x01, 0x00]) {
            return Some(ImageFormat::Ico);
        }
        // SVG: an <svg> root, optionally after an XML prolog, within the first 512 bytes
        let prefix = &bytes[..bytes.len().min(512)];
        let text = match std::str::from_utf8(prefix) {
            Ok(s) => s,
            // The 512-byte cut may split a multi-byte character.
            Err(e) => std::str::from_utf8(&prefix[..e.valid_up_to()]).unwrap_or(""),
        };
        let trimmed = text.trim_start_matches('\u{FEFF}').trim_start();
        if trimmed.starts_with("<svg") || (trimmed.starts_with("<?xml") && text.contains("<svg")) {
            return Some(ImageFormat::Svg);
        }
        None
    }

    /// Detects AVIF, HEIC and Canon CR3 from an ISO-BMFF `ftyp` box, checking the
    /// major brand first and then the compatible brands (many AVIF files use the
    /// generic `mif1` major brand and only list `avif` as a compatible brand).
    fn detect_isobmff(bytes: &[u8]) -> Option<ImageFormat> {
        if bytes.len() < 12 || &bytes[4..8] != b"ftyp" {
            return None;
        }
        let box_len = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        let end = box_len.clamp(12, bytes.len().min(256));

        let major = &bytes[8..12];
        let compatible = bytes.get(16..end).unwrap_or(&[]).chunks_exact(4);

        let mut heif_family = false;
        for brand in std::iter::once(major).chain(compatible) {
            match brand {
                b"avif" | b"avis" | b"MA1B" | b"MA1A" => return Some(ImageFormat::Avif),
                b"crx " | b"cr3 " | b"CAnO" => return Some(ImageFormat::Raw),
                b"heic" | b"heix" | b"hevc" | b"hevx" | b"heim" | b"heis" | b"mif1" | b"msf1" => {
                    heif_family = true
                }
                _ => {}
            }
        }
        heif_family.then_some(ImageFormat::Heic)
    }
}

impl fmt::Display for ImageFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

impl FromStr for ImageFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let clean = s.trim().to_ascii_lowercase();
        match clean.as_str() {
            "jpg" | "jpeg" => Ok(ImageFormat::Jpeg),
            "jfif" => Ok(ImageFormat::Jfif),
            "png" => Ok(ImageFormat::Png),
            "webp" => Ok(ImageFormat::Webp),
            "avif" => Ok(ImageFormat::Avif),
            "svg" => Ok(ImageFormat::Svg),
            "gif" => Ok(ImageFormat::Gif),
            "ico" => Ok(ImageFormat::Ico),
            "bmp" => Ok(ImageFormat::Bmp),
            "tiff" | "tif" => Ok(ImageFormat::Tiff),
            "tga" => Ok(ImageFormat::Tga),
            "dds" => Ok(ImageFormat::Dds),
            "hdr" => Ok(ImageFormat::Hdr),
            "exr" => Ok(ImageFormat::Exr),
            "heic" | "heif" => Ok(ImageFormat::Heic),
            "raw" | "cr2" | "cr3" | "nef" | "arw" | "dng" => Ok(ImageFormat::Raw),
            "pdf" => Ok(ImageFormat::Pdf),
            _ => Err(format!("Unsupported format '{}'", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_extensions() {
        assert_eq!(ImageFormat::from_extension("jpg"), Some(ImageFormat::Jpeg));
        assert_eq!(ImageFormat::from_extension("jfif"), Some(ImageFormat::Jfif));
        assert_eq!(
            ImageFormat::from_extension(".JPEG"),
            Some(ImageFormat::Jpeg)
        );
        assert_eq!(ImageFormat::from_extension("png"), Some(ImageFormat::Png));
        assert_eq!(ImageFormat::from_extension("webp"), Some(ImageFormat::Webp));
        assert_eq!(ImageFormat::from_extension("avif"), Some(ImageFormat::Avif));
        assert_eq!(ImageFormat::from_extension("svg"), Some(ImageFormat::Svg));
        assert_eq!(ImageFormat::from_extension("gif"), Some(ImageFormat::Gif));
        assert_eq!(ImageFormat::from_extension("ico"), Some(ImageFormat::Ico));
        assert_eq!(ImageFormat::from_extension("bmp"), Some(ImageFormat::Bmp));
        assert_eq!(ImageFormat::from_extension("tiff"), Some(ImageFormat::Tiff));
        assert_eq!(ImageFormat::from_extension("tif"), Some(ImageFormat::Tiff));
        assert_eq!(ImageFormat::from_extension("tga"), Some(ImageFormat::Tga));
        assert_eq!(ImageFormat::from_extension("dds"), Some(ImageFormat::Dds));
        assert_eq!(ImageFormat::from_extension("hdr"), Some(ImageFormat::Hdr));
        assert_eq!(ImageFormat::from_extension("exr"), Some(ImageFormat::Exr));
        assert_eq!(ImageFormat::from_extension("heic"), Some(ImageFormat::Heic));
        assert_eq!(ImageFormat::from_extension("heif"), Some(ImageFormat::Heic));
        assert_eq!(ImageFormat::from_extension("cr2"), Some(ImageFormat::Raw));
        assert_eq!(ImageFormat::from_extension("cr3"), Some(ImageFormat::Raw));
        assert_eq!(ImageFormat::from_extension("nef"), Some(ImageFormat::Raw));
        assert_eq!(ImageFormat::from_extension("arw"), Some(ImageFormat::Raw));
        assert_eq!(ImageFormat::from_extension("dng"), Some(ImageFormat::Raw));
        assert_eq!(ImageFormat::from_extension("pdf"), Some(ImageFormat::Pdf));
        assert_eq!(ImageFormat::from_extension("xyz"), None);
    }

    #[test]
    fn test_magic_bytes() {
        let jpeg_magic = [0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
        assert_eq!(
            ImageFormat::detect_from_bytes(&jpeg_magic),
            Some(ImageFormat::Jpeg)
        );

        let png_magic = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00];
        assert_eq!(
            ImageFormat::detect_from_bytes(&png_magic),
            Some(ImageFormat::Png)
        );

        let webp_magic = b"RIFF\x20\x00\x00\x00WEBPVP8 ";
        assert_eq!(
            ImageFormat::detect_from_bytes(webp_magic),
            Some(ImageFormat::Webp)
        );

        let bmp_magic = [0x42, 0x4D, 0x00, 0x10];
        assert_eq!(
            ImageFormat::detect_from_bytes(&bmp_magic),
            Some(ImageFormat::Bmp)
        );

        let tiff_magic = [0x49, 0x49, 0x2A, 0x00, 0x08, 0x00];
        assert_eq!(
            ImageFormat::detect_from_bytes(&tiff_magic),
            Some(ImageFormat::Tiff)
        );

        let pdf_magic = b"%PDF-1.7\r\n";
        assert_eq!(
            ImageFormat::detect_from_bytes(pdf_magic),
            Some(ImageFormat::Pdf)
        );

        let jfif_magic = b"\xFF\xD8\xFF\xE0\x00\x10JFIF\x00";
        assert_eq!(
            ImageFormat::detect_from_bytes(jfif_magic),
            Some(ImageFormat::Jpeg)
        );
    }

    fn ftyp(major: &[u8; 4], compatible: &[&[u8; 4]]) -> Vec<u8> {
        let len = 16 + compatible.len() * 4;
        let mut out = (len as u32).to_be_bytes().to_vec();
        out.extend_from_slice(b"ftyp");
        out.extend_from_slice(major);
        out.extend_from_slice(&[0, 0, 0, 0]);
        for brand in compatible {
            out.extend_from_slice(*brand);
        }
        out
    }

    #[test]
    fn test_isobmff_brands() {
        let avif = ftyp(b"avif", &[b"mif1", b"miaf"]);
        assert_eq!(
            ImageFormat::detect_from_bytes(&avif),
            Some(ImageFormat::Avif)
        );

        // Generic major brand with AVIF only listed as a compatible brand.
        let avif_mif1 = ftyp(b"mif1", &[b"avif", b"miaf"]);
        assert_eq!(
            ImageFormat::detect_from_bytes(&avif_mif1),
            Some(ImageFormat::Avif)
        );

        let heic = ftyp(b"heic", &[b"mif1", b"heic"]);
        assert_eq!(
            ImageFormat::detect_from_bytes(&heic),
            Some(ImageFormat::Heic)
        );

        let cr3 = ftyp(b"crx ", &[b"isom"]);
        assert_eq!(ImageFormat::detect_from_bytes(&cr3), Some(ImageFormat::Raw));

        let mp4 = ftyp(b"isom", &[b"mp41"]);
        assert_eq!(ImageFormat::detect_from_bytes(&mp4), None);
    }

    #[test]
    fn test_svg_detection() {
        assert_eq!(
            ImageFormat::detect_from_bytes(b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>"),
            Some(ImageFormat::Svg)
        );
        assert_eq!(
            ImageFormat::detect_from_bytes(b"<?xml version=\"1.0\"?>\n<svg></svg>"),
            Some(ImageFormat::Svg)
        );
        // Plain XML that is not an SVG document.
        assert_eq!(
            ImageFormat::detect_from_bytes(b"<?xml version=\"1.0\"?><plist></plist>"),
            None
        );
    }
}
