use crate::codecs::jpeg::JpegCodec;
use crate::codecs::{check_decoded, EncodeOptions, ImageDecoder, ImageEncoder};
use crate::decoded::DecodedImage;
use img_converter_common::{ConversionError, DecodeLimits, ImageFormat, MetadataPolicy};
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref};

/// Resolution at which images are placed on PDF pages (pixels per inch).
const IMAGE_PLACEMENT_DPI: f32 = 300.0;

/// JPEG quality used for images embedded in generated PDFs.
const EMBEDDED_JPEG_QUALITY: u8 = 95;

/// Incrementally builds a PDF with one image per page, so large batches only
/// keep the compressed page data in memory.
pub struct PdfBuilder {
    pdf: Pdf,
    catalog_id: Ref,
    pages_id: Ref,
    page_refs: Vec<Ref>,
    next_ref: i32,
}

impl Default for PdfBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl PdfBuilder {
    pub fn new() -> Self {
        Self {
            pdf: Pdf::new(),
            catalog_id: Ref::new(1),
            pages_id: Ref::new(2),
            page_refs: Vec::new(),
            next_ref: 3,
        }
    }

    fn alloc_ref(&mut self) -> Ref {
        let r = Ref::new(self.next_ref);
        self.next_ref += 1;
        r
    }

    pub fn page_count(&self) -> usize {
        self.page_refs.len()
    }

    /// Appends `image` as a new page. Transparency must already be composited:
    /// pages are embedded as JPEG (DCT) streams without an alpha channel.
    pub fn add_page(&mut self, image: &DecodedImage) -> Result<(), ConversionError> {
        let page_id = self.alloc_ref();
        let content_id = self.alloc_ref();
        let image_id = self.alloc_ref();
        self.page_refs.push(page_id);

        let options = EncodeOptions {
            quality: EMBEDDED_JPEG_QUALITY,
            metadata: MetadataPolicy::Strip,
            ..Default::default()
        };
        let jpeg = JpegCodec::encode(image, &options)?;

        let width_pt = image.width as f32 / IMAGE_PLACEMENT_DPI * 72.0;
        let height_pt = image.height as f32 / IMAGE_PLACEMENT_DPI * 72.0;
        let name_str = format!("Im{}", self.page_refs.len());
        let image_name = Name(name_str.as_bytes());

        let mut page = self.pdf.page(page_id);
        page.parent(self.pages_id);
        page.media_box(Rect::new(0.0, 0.0, width_pt, height_pt));
        page.contents(content_id);
        page.resources().x_objects().pair(image_name, image_id);
        page.finish();

        let mut content = Content::new();
        content.save_state();
        content.transform([width_pt, 0.0, 0.0, height_pt, 0.0, 0.0]);
        content.x_object(image_name);
        content.restore_state();
        self.pdf.stream(content_id, &content.finish());

        let mut xobject = self.pdf.image_xobject(image_id, &jpeg);
        xobject.width(image.width as i32);
        xobject.height(image.height as i32);
        xobject.color_space().device_rgb();
        xobject.bits_per_component(8);
        xobject.filter(Filter::DctDecode);
        xobject.finish();

        Ok(())
    }

    pub fn finish(mut self) -> Result<Vec<u8>, ConversionError> {
        if self.page_refs.is_empty() {
            return Err(ConversionError::EncodeError(
                "Cannot create a PDF without pages".to_string(),
            ));
        }
        self.pdf.catalog(self.catalog_id).pages(self.pages_id);
        let count = self.page_refs.len() as i32;
        self.pdf
            .pages(self.pages_id)
            .kids(self.page_refs.iter().copied())
            .count(count);
        Ok(self.pdf.finish())
    }
}

pub struct PdfCodec;

impl PdfCodec {
    /// Renders every page at `dpi` and hands each one to `on_page(index, total, page)`
    /// as soon as it is ready. Pages that would exceed `limits` are rendered at a
    /// proportionally lower resolution instead of failing.
    ///
    /// Uses the Windows built-in PDF renderer (`Windows.Data.Pdf`); if unavailable it
    /// falls back to extracting the images embedded in each page.
    pub fn render_pages<F>(
        bytes: &[u8],
        dpi: u32,
        limits: &DecodeLimits,
        mut on_page: F,
    ) -> Result<usize, ConversionError>
    where
        F: FnMut(usize, usize, DecodedImage) -> Result<(), ConversionError>,
    {
        limits
            .check_file_size(bytes.len() as u64)
            .map_err(ConversionError::LimitExceeded)?;

        let rendered = Self::render_pages_winrt(bytes, dpi, limits, &mut on_page);
        match rendered {
            Ok(count) if count > 0 => return Ok(count),
            // A callback error (e.g. disk full) must not trigger the fallback.
            Err(RenderError::Callback(e)) => return Err(e),
            _ => {}
        }

        let count = Self::extract_embedded_images(bytes, limits, &mut on_page)?;
        if count == 0 {
            return Err(ConversionError::DecodeError(
                "No renderable pages found in PDF document".to_string(),
            ));
        }
        Ok(count)
    }

    fn render_pages_winrt<F>(
        bytes: &[u8],
        dpi: u32,
        limits: &DecodeLimits,
        on_page: &mut F,
    ) -> Result<usize, RenderError>
    where
        F: FnMut(usize, usize, DecodedImage) -> Result<(), ConversionError>,
    {
        use windows::Data::Pdf::{PdfDocument, PdfPageRenderOptions};
        use windows::Storage::Streams::{DataReader, DataWriter, InMemoryRandomAccessStream};
        use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};

        unsafe {
            let _ = RoInitialize(RO_INIT_MULTITHREADED);
        }

        let stream = InMemoryRandomAccessStream::new()?;
        let writer = DataWriter::CreateDataWriter(&stream)?;
        writer.WriteBytes(bytes)?;
        writer.StoreAsync()?.get()?;
        writer.FlushAsync()?.get()?;
        stream.Seek(0)?;

        let doc = PdfDocument::LoadFromStreamAsync(&stream)?.get()?;
        let page_count = doc.PageCount()? as usize;
        let mut rendered = 0;

        for page_idx in 0..page_count {
            let page = doc.GetPage(page_idx as u32)?;
            let size = page.Size()?;
            let (target_w, target_h) = fit_to_limits(
                size.Width * dpi.max(72) as f32 / 72.0,
                size.Height * dpi.max(72) as f32 / 72.0,
                limits,
            );

            let render_stream = InMemoryRandomAccessStream::new()?;
            let options = PdfPageRenderOptions::new()?;
            options.SetDestinationWidth(target_w)?;
            options.SetDestinationHeight(target_h)?;
            page.RenderWithOptionsToStreamAsync(&render_stream, &options)?
                .get()?;

            let stream_size = render_stream.Size()? as u32;
            if stream_size == 0 {
                continue;
            }
            render_stream.Seek(0)?;
            let reader = DataReader::CreateDataReader(&render_stream)?;
            reader.LoadAsync(stream_size)?.get()?;
            let mut encoded = vec![0u8; stream_size as usize];
            reader.ReadBytes(&mut encoded)?;

            let Ok(img) = image::load_from_memory(&encoded) else {
                continue;
            };
            let mut decoded = DecodedImage::from_dynamic(img, ImageFormat::Pdf);
            decoded.drop_opaque_alpha();
            check_decoded(&decoded, limits).map_err(RenderError::Callback)?;
            on_page(page_idx, page_count, decoded).map_err(RenderError::Callback)?;
            rendered += 1;
        }

        Ok(rendered)
    }

    /// Fallback: extracts embedded image XObjects (typically scanned pages).
    fn extract_embedded_images<F>(
        bytes: &[u8],
        limits: &DecodeLimits,
        on_page: &mut F,
    ) -> Result<usize, ConversionError>
    where
        F: FnMut(usize, usize, DecodedImage) -> Result<(), ConversionError>,
    {
        let Ok(doc) = lopdf::Document::load_mem(bytes) else {
            return Ok(0);
        };

        let mut images = Vec::new();
        for (_, page_id) in doc.get_pages() {
            if let Ok(page_images) = doc.get_page_images(page_id) {
                images.extend(page_images.into_iter().map(|img| img.content.to_vec()));
            }
        }

        let total = images.len();
        let mut count = 0;
        for content in images {
            let Ok(img) = image::load_from_memory(&content) else {
                continue;
            };
            let decoded = DecodedImage::from_dynamic(img, ImageFormat::Pdf);
            check_decoded(&decoded, limits)?;
            on_page(count, total, decoded)?;
            count += 1;
        }
        Ok(count)
    }
}

/// Error from the WinRT renderer: either the renderer itself failed (fall back to
/// image extraction) or the page callback failed (abort).
enum RenderError {
    Renderer,
    Callback(ConversionError),
}

impl From<windows::core::Error> for RenderError {
    fn from(_: windows::core::Error) -> Self {
        RenderError::Renderer
    }
}

/// Scales a page size down (keeping the aspect ratio) so it fits within `limits`.
fn fit_to_limits(width: f32, height: f32, limits: &DecodeLimits) -> (u32, u32) {
    let width = width.max(1.0);
    let height = height.max(1.0);
    let mut scale = 1.0f32
        .min(limits.max_width as f32 / width)
        .min(limits.max_height as f32 / height);
    let pixels = (width * scale) as f64 * (height * scale) as f64;
    let max_pixels = (limits.max_pixels as f64).min(limits.max_memory_bytes as f64 / 4.0);
    if pixels > max_pixels {
        scale *= (max_pixels / pixels).sqrt() as f32;
    }
    (
        ((width * scale).floor() as u32).max(1),
        ((height * scale).floor() as u32).max(1),
    )
}

impl ImageDecoder for PdfCodec {
    /// Decodes the first page at 300 DPI.
    fn decode(bytes: &[u8], limits: &DecodeLimits) -> Result<DecodedImage, ConversionError> {
        let mut first = None;
        Self::render_pages(bytes, 300, limits, |_, _, page| {
            if first.is_none() {
                first = Some(page);
            }
            Ok(())
        })?;
        first.ok_or_else(|| ConversionError::DecodeError("PDF document has no pages".to_string()))
    }
}

impl ImageEncoder for PdfCodec {
    fn encode(image: &DecodedImage, _options: &EncodeOptions) -> Result<Vec<u8>, ConversionError> {
        let mut builder = PdfBuilder::new();
        builder.add_page(image)?;
        builder.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fit_to_limits() {
        let limits = DecodeLimits::default();
        assert_eq!(fit_to_limits(2480.0, 3508.0, &limits), (2480, 3508));

        // A0 at 1200 DPI is far beyond 16k px: shrink, keeping the aspect ratio.
        let (w, h) = fit_to_limits(39_732.0, 56_173.0, &limits);
        assert!(w <= limits.max_width && h <= limits.max_height);
        assert!((w as f32 / h as f32 - 39_732.0 / 56_173.0).abs() < 0.01);
        assert!(w as u64 * h as u64 * 4 <= limits.max_memory_bytes);
    }

    #[test]
    fn test_builder_requires_pages() {
        assert!(PdfBuilder::new().finish().is_err());
    }
}
