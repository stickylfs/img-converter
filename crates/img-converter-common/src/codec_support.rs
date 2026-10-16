//! Detection of optional Windows Imaging Component (WIC) codecs.

use serde::{Deserialize, Serialize};
use windows::core::Interface;
use windows::Win32::Graphics::Imaging::{
    CLSID_WICImagingFactory, GUID_WICPixelFormat32bppRGBA, IWICBitmapDecoderInfo,
    IWICImagingFactory, WICBitmapDitherTypeNone, WICBitmapPaletteTypeCustom,
    WICComponentEnumerateDefault, WICDecodeMetadataCacheOnDemand, WICDecoder,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED,
};

/// Microsoft Store page of the "AV1 Video Extension" (adds AVIF decoding).
pub const AV1_EXTENSION_STORE_URL: &str = "ms-windows-store://pdp/?ProductId=9MVZQVXJBQ9V";

/// Microsoft Store page of the "Raw Image Extension" (LibRaw-based codec for
/// camera RAW files; it updates the codec built into Windows 10 1903+ and 11).
pub const RAW_EXTENSION_STORE_URL: &str = "ms-windows-store://pdp/?ProductId=9NCTDW2W1BH8";

/// Microsoft Store page of the "HEIF Image Extensions" (HEIC/HEIF decoding,
/// together with the "HEVC Video Extensions").
pub const HEIF_EXTENSION_STORE_URL: &str = "ms-windows-store://pdp/?ProductId=9PMMSR1CGPWG";

/// Optional Windows codecs used to read some input formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WindowsCodec {
    /// "AV1 Video Extension": AVIF.
    Av1,
    /// "HEIF Image Extensions" (+ "HEVC Video Extensions"): HEIC/HEIF.
    Heif,
    /// "Raw Image Extension": camera RAW.
    Raw,
}

impl WindowsCodec {
    /// Name of the extension in the Microsoft Store.
    pub fn store_name(self) -> &'static str {
        match self {
            WindowsCodec::Av1 => "AV1 Video Extension",
            WindowsCodec::Heif => "HEIF Image Extensions",
            WindowsCodec::Raw => "Raw Image Extension",
        }
    }

    pub fn store_url(self) -> &'static str {
        match self {
            WindowsCodec::Av1 => AV1_EXTENSION_STORE_URL,
            WindowsCodec::Heif => HEIF_EXTENSION_STORE_URL,
            WindowsCodec::Raw => RAW_EXTENSION_STORE_URL,
        }
    }

    /// Whether Windows has a decoder for the format. For RAW, `.cr3` tells the
    /// current LibRaw-based codec apart from the legacy camera codec.
    ///
    /// For AV1, a registered `.avif` decoder is not enough: the HEIF extension
    /// registers one that fails without the AV1 codec, so a tiny AVIF is opened.
    pub fn is_installed(self) -> bool {
        match self {
            WindowsCodec::Av1 => wic_can_decode(".avif") && wic_can_open(PROBE_AVIF),
            WindowsCodec::Heif => wic_can_decode(".heic"),
            WindowsCodec::Raw => wic_can_decode(".cr3"),
        }
    }
}

/// 8x8 white 8-bit AVIF, produced by img-converter's own encoder.
const PROBE_AVIF: &[u8] = include_bytes!("probe.avif");

/// Returns `true` when WIC can decode the first frame of `bytes`.
fn wic_can_open(bytes: &[u8]) -> bool {
    unsafe {
        let com_initialized = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let opened = open_first_frame(bytes).is_ok();
        if com_initialized {
            CoUninitialize();
        }
        opened
    }
}

unsafe fn open_first_frame(bytes: &[u8]) -> windows::core::Result<()> {
    let factory: IWICImagingFactory =
        CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
    let stream = factory.CreateStream()?;
    stream.InitializeFromMemory(bytes)?;
    let decoder = factory.CreateDecoderFromStream(
        &stream,
        std::ptr::null(),
        WICDecodeMetadataCacheOnDemand,
    )?;
    let frame = decoder.GetFrame(0)?;
    let converter = factory.CreateFormatConverter()?;
    converter.Initialize(
        &frame,
        &GUID_WICPixelFormat32bppRGBA,
        WICBitmapDitherTypeNone,
        None,
        0.0,
        WICBitmapPaletteTypeCustom,
    )?;
    let (mut width, mut height) = (0, 0);
    converter.GetSize(&mut width, &mut height)?;
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    converter.CopyPixels(std::ptr::null(), width * 4, &mut pixels)
}

/// Returns `true` when an installed WIC decoder handles files with `extension`
/// (e.g. `".avif"`).
pub fn wic_can_decode(extension: &str) -> bool {
    unsafe {
        let com_initialized = CoInitializeEx(None, COINIT_MULTITHREADED).is_ok();
        let found = find_decoder(extension).unwrap_or(false);
        if com_initialized {
            CoUninitialize();
        }
        found
    }
}

unsafe fn find_decoder(extension: &str) -> windows::core::Result<bool> {
    let factory: IWICImagingFactory =
        CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER)?;
    let components = factory
        .CreateComponentEnumerator(WICDecoder.0 as u32, WICComponentEnumerateDefault.0 as u32)?;

    let wanted = extension.to_ascii_lowercase();
    loop {
        let mut items = [None];
        let mut fetched = 0;
        if components.Next(&mut items, Some(&mut fetched)).is_err() || fetched == 0 {
            return Ok(false);
        }
        let Some(info) = items[0]
            .take()
            .and_then(|u| u.cast::<IWICBitmapDecoderInfo>().ok())
        else {
            continue;
        };

        // Comma-separated list such as ".jpeg,.jpe,.jpg,.jfif,.exif".
        let mut buf = [0u16; 1024];
        let mut len = 0;
        if info.GetFileExtensions(&mut buf, &mut len).is_err() || len == 0 {
            continue;
        }
        let len = (len as usize).min(buf.len());
        let extensions = String::from_utf16_lossy(&buf[..len]);
        let extensions = extensions.trim_end_matches(char::from(0));
        if extensions
            .to_ascii_lowercase()
            .split(',')
            .any(|e| e.trim() == wanted)
        {
            return Ok(true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_codecs_are_detected() {
        // PNG and JPEG decoders ship with every Windows installation.
        assert!(wic_can_decode(".png"));
        assert!(wic_can_decode(".JPG"));
        assert!(!wic_can_decode(".not-an-image"));
    }

    #[test]
    fn test_av1_probe_matches_decoder_registration() {
        // The probe can only succeed when a `.avif` decoder is registered.
        if WindowsCodec::Av1.is_installed() {
            assert!(wic_can_decode(".avif"));
        }
        assert!(!wic_can_open(b"not an image"));
    }
}
