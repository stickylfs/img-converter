use crate::codecs::check_decoded;
use crate::decoded::{DecodedImage, PixelFormat};
use img_converter_common::{ConversionError, DecodeLimits, ImageFormat};
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree};
use std::sync::{Arc, OnceLock};

pub struct SvgCodec;

/// System fonts, loaded once per process so SVG `<text>` elements render.
fn system_fonts() -> Arc<resvg::usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<resvg::usvg::fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = resvg::usvg::fontdb::Database::new();
            db.load_system_fonts();
            Arc::new(db)
        })
        .clone()
}

impl SvgCodec {
    /// Rasterizes an SVG document at `scale` times its intrinsic size.
    pub fn decode(
        bytes: &[u8],
        limits: &DecodeLimits,
        scale: f32,
    ) -> Result<DecodedImage, ConversionError> {
        limits
            .check_file_size(bytes.len() as u64)
            .map_err(ConversionError::LimitExceeded)?;

        let options = Options {
            fontdb: system_fonts(),
            ..Options::default()
        };
        let tree = Tree::from_data(bytes, &options)
            .map_err(|e| ConversionError::DecodeError(e.to_string()))?;

        let scale = if scale.is_finite() {
            scale.clamp(0.1, 10.0)
        } else {
            1.0
        };
        let size = tree.size();
        let width = ((size.width() * scale).ceil() as u32).max(1);
        let height = ((size.height() * scale).ceil() as u32).max(1);

        limits
            .check_dimensions(width, height)
            .map_err(ConversionError::LimitExceeded)?;

        let mut pixmap = Pixmap::new(width, height).ok_or_else(|| {
            ConversionError::DecodeError("Failed to allocate SVG raster pixmap".to_string())
        })?;
        resvg::render(
            &tree,
            Transform::from_scale(scale, scale),
            &mut pixmap.as_mut(),
        );

        // tiny-skia produces premultiplied alpha; convert to straight RGBA.
        let mut pixels = pixmap.take();
        for px in pixels.chunks_exact_mut(4) {
            let a = px[3] as u32;
            if a > 0 && a < 255 {
                for c in &mut px[..3] {
                    *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
                }
            }
        }

        let decoded =
            DecodedImage::new(width, height, PixelFormat::Rgba8, pixels, ImageFormat::Svg);
        check_decoded(&decoded, limits)?;
        Ok(decoded)
    }
}
