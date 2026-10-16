use crate::atomic_io::reserve_destination;
use crate::codecs::avif::AvifCodec;
use crate::codecs::bmp::BmpCodec;
use crate::codecs::dds::DdsCodec;
use crate::codecs::exr::ExrCodec;
use crate::codecs::gif::GifCodec;
use crate::codecs::hdr::HdrCodec;
use crate::codecs::heic::HeicCodec;
use crate::codecs::ico::IcoCodec;
use crate::codecs::jpeg::JpegCodec;
use crate::codecs::pdf::{PdfBuilder, PdfCodec};
use crate::codecs::png::PngCodec;
use crate::codecs::raw::RawCodec;
use crate::codecs::svg::SvgCodec;
use crate::codecs::tga::TgaCodec;
use crate::codecs::tiff::TiffCodec;
use crate::codecs::webp::WebpCodec;
use crate::codecs::{EncodeOptions, ImageDecoder, ImageEncoder};
use crate::decoded::DecodedImage;
use crate::orientation::apply_exif_orientation;
use crate::transparency::composite_transparency;
use img_converter_common::{
    ConversionError, ConversionMatrix, ConversionRequest, ConversionSummary, DecodeLimits,
    ImageFormat, SingleFileResult, TransparencyPolicy,
};
use rayon::prelude::*;
use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

/// Upper bound on parallel conversions: each one may hold several hundred MB of
/// decoded pixels, so batch size must not scale memory use with the CPU count.
const MAX_PARALLEL_CONVERSIONS: usize = 4;

/// Resolution used when PDF pages are rasterized to be merged into a new PDF.
const PDF_MERGE_DPI: u32 = 200;

pub struct ImageEngine;

/// File name stem used for outputs (lossy for non-UTF-16 names).
fn file_stem(path: &Path, fallback: &str) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn elapsed_ms(start: Instant) -> u64 {
    start.elapsed().as_millis() as u64
}

/// Reads an input file after validating its size against `limits`.
fn read_input(path: &Path, limits: &DecodeLimits) -> Result<Vec<u8>, ConversionError> {
    let metadata = fs::metadata(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => {
            ConversionError::InvalidArgument(format!("File not found: {e}"))
        }
        _ => ConversionError::IoError(format!("Cannot open file: {e}")),
    })?;
    if !metadata.is_file() {
        return Err(ConversionError::InvalidArgument("Not a file".to_string()));
    }
    limits
        .check_file_size(metadata.len())
        .map_err(ConversionError::LimitExceeded)?;
    fs::read(path).map_err(|e| ConversionError::IoError(format!("Cannot read file: {e}")))
}

/// Compares file names "naturally" so `page2.png` sorts before `page10.png`.
fn natural_cmp(a: &Path, b: &Path) -> Ordering {
    let a = a.to_string_lossy().to_lowercase();
    let b = b.to_string_lossy().to_lowercase();
    let (mut ai, mut bi) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let take_number = |it: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut digits = String::new();
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                        digits.push(c);
                        it.next();
                    }
                    digits
                };
                let (na, nb) = (take_number(&mut ai), take_number(&mut bi));
                let (ta, tb) = (na.trim_start_matches('0'), nb.trim_start_matches('0'));
                let ord = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(x), Some(y)) => {
                if x != y {
                    return x.cmp(&y);
                }
                ai.next();
                bi.next();
            }
        }
    }
}

impl ImageEngine {
    /// Detects the format of an image from its content, using the extension to
    /// disambiguate (camera RAW files are TIFF-based, `.jfif` is plain JPEG).
    pub fn detect_format(path: &Path, header: &[u8]) -> Result<ImageFormat, ConversionError> {
        let by_extension = ImageFormat::from_path(path);
        if by_extension == Some(ImageFormat::Raw) {
            return Ok(ImageFormat::Raw);
        }
        match ImageFormat::detect_from_bytes(header) {
            Some(ImageFormat::Jpeg) if by_extension == Some(ImageFormat::Jfif) => {
                Ok(ImageFormat::Jfif)
            }
            Some(detected) => Ok(detected),
            None => by_extension
                .ok_or_else(|| ConversionError::UnsupportedFormat(path.display().to_string())),
        }
    }

    /// Decodes an image of any supported input format (first page for PDF).
    pub fn decode_image(
        format: ImageFormat,
        bytes: &[u8],
        limits: &DecodeLimits,
        scale: Option<f32>,
    ) -> Result<DecodedImage, ConversionError> {
        match format {
            ImageFormat::Jpeg | ImageFormat::Jfif => JpegCodec::decode(bytes, limits),
            ImageFormat::Png => PngCodec::decode(bytes, limits),
            ImageFormat::Webp => WebpCodec::decode(bytes, limits),
            ImageFormat::Avif => AvifCodec::decode(bytes, limits),
            ImageFormat::Svg => SvgCodec::decode(bytes, limits, scale.unwrap_or(1.0)),
            ImageFormat::Gif => GifCodec::decode(bytes, limits),
            ImageFormat::Ico => IcoCodec::decode(bytes, limits),
            ImageFormat::Bmp => BmpCodec::decode(bytes, limits),
            ImageFormat::Tiff => TiffCodec::decode(bytes, limits),
            ImageFormat::Tga => TgaCodec::decode(bytes, limits),
            ImageFormat::Dds => DdsCodec::decode(bytes, limits),
            ImageFormat::Hdr => HdrCodec::decode(bytes, limits),
            ImageFormat::Exr => ExrCodec::decode(bytes, limits),
            ImageFormat::Heic => HeicCodec::decode(bytes, limits),
            ImageFormat::Raw => RawCodec::decode(bytes, limits),
            ImageFormat::Pdf => PdfCodec::decode(bytes, limits),
        }
    }

    /// Encodes an image to any supported output format.
    pub fn encode_image(
        image: &DecodedImage,
        target_format: ImageFormat,
        options: &EncodeOptions,
    ) -> Result<Vec<u8>, ConversionError> {
        match target_format {
            ImageFormat::Jpeg | ImageFormat::Jfif => JpegCodec::encode(image, options),
            ImageFormat::Png => PngCodec::encode(image, options),
            ImageFormat::Webp => WebpCodec::encode(image, options),
            ImageFormat::Avif => AvifCodec::encode(image, options),
            ImageFormat::Gif => GifCodec::encode(image, options),
            ImageFormat::Ico => IcoCodec::encode(image, options),
            ImageFormat::Bmp => BmpCodec::encode(image, options),
            ImageFormat::Tiff => TiffCodec::encode(image, options),
            ImageFormat::Tga => TgaCodec::encode(image, options),
            ImageFormat::Hdr => HdrCodec::encode(image, options),
            ImageFormat::Exr => ExrCodec::encode(image, options),
            ImageFormat::Pdf => PdfCodec::encode(image, options),
            ImageFormat::Svg | ImageFormat::Dds | ImageFormat::Heic | ImageFormat::Raw => Err(
                ConversionError::EncodeError(format!("{} is an input-only format", target_format)),
            ),
        }
    }

    fn encode_options(request: &ConversionRequest) -> EncodeOptions {
        EncodeOptions {
            quality: request.quality.unwrap_or(90),
            avif_speed: request.avif_speed.unwrap_or(6),
            ico_multi_resolution: request.ico_multi_resolution,
            metadata: request.metadata,
        }
    }

    /// Normalizes a decoded image for `target_format`: drops a fully opaque alpha
    /// channel, applies the EXIF orientation and composites transparency onto the
    /// matte color when the target has no alpha support.
    fn prepare(
        mut image: DecodedImage,
        target_format: ImageFormat,
        transparency: TransparencyPolicy,
    ) -> DecodedImage {
        image.drop_opaque_alpha();
        let image = apply_exif_orientation(image);
        if !target_format.supports_alpha() && image.has_alpha() {
            composite_transparency(&image, transparency)
        } else {
            image
        }
    }

    fn output_dir(input_path: &Path, request: &ConversionRequest) -> PathBuf {
        request.output_dir.clone().unwrap_or_else(|| {
            input_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| PathBuf::from("."))
        })
    }

    /// Converts a single file according to `request` (its `files` list is ignored).
    pub fn convert_single(input_path: &Path, request: &ConversionRequest) -> SingleFileResult {
        let start = Instant::now();
        match Self::try_convert_single(input_path, request) {
            Ok((output_path, input_bytes, output_bytes)) => SingleFileResult::success(
                input_path.to_path_buf(),
                output_path,
                input_bytes,
                output_bytes,
                elapsed_ms(start),
            ),
            Err(e) => SingleFileResult::failure(input_path.to_path_buf(), &e, elapsed_ms(start)),
        }
    }

    fn try_convert_single(
        input_path: &Path,
        request: &ConversionRequest,
    ) -> Result<(PathBuf, u64, u64), ConversionError> {
        let target_format = request.target_format;
        let raw_bytes = read_input(input_path, &request.limits)?;
        let input_len = raw_bytes.len() as u64;
        let source_format = Self::detect_format(input_path, &raw_bytes)?;

        // Mixed selections may include files already in the target format.
        if source_format == target_format {
            return Ok((input_path.to_path_buf(), input_len, input_len));
        }

        if !ConversionMatrix::is_conversion_allowed(source_format, target_format) {
            return Err(ConversionError::DisallowedConversion {
                from_format: source_format,
                to_format: target_format,
            });
        }

        let output_dir = Self::output_dir(input_path, request);
        let stem = file_stem(input_path, "image");

        // A .jfif file is a regular JPEG: copy it losslessly instead of re-encoding.
        if source_format == ImageFormat::Jfif && target_format == ImageFormat::Jpeg {
            let reservation =
                reserve_destination(&output_dir, &stem, target_format, request.collision)?;
            let path = reservation.commit(&raw_bytes)?;
            return Ok((path, input_len, input_len));
        }

        if source_format == ImageFormat::Pdf {
            return Self::convert_pdf_to_images(&raw_bytes, &output_dir, &stem, request)
                .map(|(path, written)| (path, input_len, written));
        }

        let decoded =
            Self::decode_image(source_format, &raw_bytes, &request.limits, request.scale)?;
        drop(raw_bytes);
        let prepared = Self::prepare(decoded, target_format, request.transparency);
        let encoded = Self::encode_image(&prepared, target_format, &Self::encode_options(request))?;
        drop(prepared);

        let reservation =
            reserve_destination(&output_dir, &stem, target_format, request.collision)?;
        let path = reservation.commit(&encoded)?;
        Ok((path, input_len, encoded.len() as u64))
    }

    /// Renders every PDF page to `target_format`. Multi-page documents go to a
    /// `<name>` subfolder (when enabled) as `<name>_page_01.<ext>`, ...
    /// Returns the first written file and the total bytes written.
    fn convert_pdf_to_images(
        bytes: &[u8],
        output_dir: &Path,
        stem: &str,
        request: &ConversionRequest,
    ) -> Result<(PathBuf, u64), ConversionError> {
        let options = Self::encode_options(request);
        let dpi = request.pdf_dpi.unwrap_or(300);
        let mut first_output: Option<PathBuf> = None;
        let mut written = 0u64;

        PdfCodec::render_pages(bytes, dpi, &request.limits, |index, total, page| {
            let (dir, page_stem) = if total <= 1 {
                (output_dir.to_path_buf(), stem.to_string())
            } else {
                let dir = if request.pdf_create_subfolder {
                    output_dir.join(stem)
                } else {
                    output_dir.to_path_buf()
                };
                let width = total.to_string().len().max(2);
                (dir, format!("{}_page_{:0width$}", stem, index + 1))
            };

            let prepared = Self::prepare(page, request.target_format, request.transparency);
            let encoded = Self::encode_image(&prepared, request.target_format, &options)?;
            let reservation =
                reserve_destination(&dir, &page_stem, request.target_format, request.collision)?;
            let path = reservation.commit(&encoded)?;
            written += encoded.len() as u64;
            first_output.get_or_insert(path);
            Ok(())
        })?;

        first_output
            .map(|path| (path, written))
            .ok_or_else(|| ConversionError::DecodeError("PDF document has no pages".to_string()))
    }

    /// Merges every input (all pages of PDF inputs) into one PDF named after the
    /// first file. Inputs are ordered by natural file name order.
    fn merge_into_pdf(request: &ConversionRequest) -> ConversionSummary {
        let start = Instant::now();
        let mut files = request.files.clone();
        files.sort_by(|a, b| natural_cmp(a, b));

        let mut builder = PdfBuilder::new();
        let mut failures = Vec::new();
        let mut input_bytes = 0u64;

        for file in &files {
            let file_start = Instant::now();
            let result = (|| -> Result<(), ConversionError> {
                let bytes = read_input(file, &request.limits)?;
                input_bytes += bytes.len() as u64;
                let format = Self::detect_format(file, &bytes)?;
                if format == ImageFormat::Pdf {
                    PdfCodec::render_pages(
                        &bytes,
                        PDF_MERGE_DPI,
                        &request.limits,
                        |_, _, page| {
                            let page = Self::prepare(page, ImageFormat::Pdf, request.transparency);
                            builder.add_page(&page)
                        },
                    )?;
                } else {
                    let decoded =
                        Self::decode_image(format, &bytes, &request.limits, request.scale)?;
                    let page = Self::prepare(decoded, ImageFormat::Pdf, request.transparency);
                    builder.add_page(&page)?;
                }
                Ok(())
            })();
            if let Err(e) = result {
                failures.push(SingleFileResult::failure(
                    file.clone(),
                    &e,
                    elapsed_ms(file_start),
                ));
            }
        }

        let first = &files[0];
        let merged = (|| -> Result<(PathBuf, u64), ConversionError> {
            if builder.page_count() == 0 {
                return Err(ConversionError::EncodeError(
                    "None of the selected files could be added to the PDF".to_string(),
                ));
            }
            let pdf = builder.finish()?;
            let dir = Self::output_dir(first, request);
            let reservation = reserve_destination(
                &dir,
                &file_stem(first, "document"),
                ImageFormat::Pdf,
                request.collision,
            )?;
            let len = pdf.len() as u64;
            Ok((reservation.commit(&pdf)?, len))
        })();

        let mut results = Vec::with_capacity(failures.len() + 1);
        match merged {
            Ok((path, len)) => results.push(SingleFileResult::success(
                first.clone(),
                path,
                input_bytes,
                len,
                elapsed_ms(start),
            )),
            Err(e) if failures.len() < files.len() => results.push(SingleFileResult::failure(
                first.clone(),
                &e,
                elapsed_ms(start),
            )),
            // Every input already failed individually; nothing more to report.
            Err(_) => {}
        }
        results.extend(failures);
        ConversionSummary::new(results)
    }

    /// Converts every file in `request`. Selecting several files with PDF as the
    /// target merges them into a single document.
    pub fn convert_batch(request: &ConversionRequest) -> ConversionSummary {
        if request.files.is_empty() {
            return ConversionSummary::new(Vec::new());
        }
        if request.target_format == ImageFormat::Pdf && request.files.len() > 1 {
            return Self::merge_into_pdf(request);
        }

        let threads = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            .min(MAX_PARALLEL_CONVERSIONS);
        let convert_all = || {
            request
                .files
                .par_iter()
                .map(|file| Self::convert_single(file, request))
                .collect::<Vec<_>>()
        };
        let results = match rayon::ThreadPoolBuilder::new().num_threads(threads).build() {
            Ok(pool) => pool.install(convert_all),
            Err(_) => convert_all(),
        };

        ConversionSummary::new(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_natural_order() {
        let mut files: Vec<PathBuf> = ["page10.png", "Page2.png", "page1.png", "cover.png"]
            .iter()
            .map(PathBuf::from)
            .collect();
        files.sort_by(|a, b| natural_cmp(a, b));
        let names: Vec<_> = files
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["cover.png", "page1.png", "Page2.png", "page10.png"]);
    }

    #[test]
    fn test_detect_format_uses_extension_hints() {
        let jpeg = [0xFF, 0xD8, 0xFF, 0xE0];
        let tiff = [0x49, 0x49, 0x2A, 0x00];
        let detect = |name: &str, bytes: &[u8]| ImageEngine::detect_format(Path::new(name), bytes);
        assert_eq!(detect("a.jfif", &jpeg).unwrap(), ImageFormat::Jfif);
        assert_eq!(detect("a.jpg", &jpeg).unwrap(), ImageFormat::Jpeg);
        // Mislabeled files are detected by content.
        assert_eq!(detect("a.png", &jpeg).unwrap(), ImageFormat::Jpeg);
        // TIFF-based camera RAWs keep their RAW identity.
        assert_eq!(detect("a.nef", &tiff).unwrap(), ImageFormat::Raw);
        assert_eq!(detect("a.tif", &tiff).unwrap(), ImageFormat::Tiff);
        assert!(detect("a.xyz", b"hello").is_err());
    }
}
