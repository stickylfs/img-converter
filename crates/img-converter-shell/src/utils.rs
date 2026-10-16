use img_converter_common::{ConversionMatrix, ImageFormat, IniConfig};
use std::path::PathBuf;
use windows::core::{HRESULT, PWSTR};
use windows::Win32::Foundation::E_OUTOFMEMORY;
use windows::Win32::System::Com::CoTaskMemAlloc;
use windows::Win32::UI::Shell::{IShellItemArray, SIGDN_FILESYSPATH};

/// Allocates and copies a UTF-16 string using CoTaskMemAlloc for Windows COM return values.
pub unsafe fn alloc_co_task_str(s: &str) -> Result<PWSTR, HRESULT> {
    let wide: Vec<u16> = s.encode_utf16().chain(std::iter::once(0)).collect();
    let bytes = wide.len() * std::mem::size_of::<u16>();
    let ptr = CoTaskMemAlloc(bytes) as *mut u16;
    if ptr.is_null() {
        return Err(E_OUTOFMEMORY);
    }
    std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
    Ok(PWSTR(ptr))
}

/// Extracts file paths from an IShellItemArray.
pub unsafe fn get_paths_from_item_array(items: Option<&IShellItemArray>) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let Some(item_array) = items else {
        return paths;
    };

    let count = match item_array.GetCount() {
        Ok(c) => c,
        Err(_) => return paths,
    };

    for i in 0..count {
        if let Ok(item) = item_array.GetItemAt(i) {
            if let Ok(path_ptr) = item.GetDisplayName(SIGDN_FILESYSPATH) {
                let path_str = path_ptr.to_string().unwrap_or_default();
                if !path_str.is_empty() {
                    paths.push(PathBuf::from(path_str));
                }
                windows::Win32::System::Com::CoTaskMemFree(Some(path_ptr.as_ptr() as *const _));
            }
        }
    }

    paths
}

/// Computes the intersection of available target formats for a list of file paths with whitelist filtering.
pub fn get_available_targets_for_files(paths: &[PathBuf]) -> Vec<ImageFormat> {
    if paths.is_empty() {
        return Vec::new();
    }

    let mut formats = Vec::with_capacity(paths.len());
    for p in paths {
        if let Some(fmt) = ImageFormat::from_path(p) {
            formats.push(fmt);
        } else {
            // Non-supported file in selection -> hide context menu
            return Vec::new();
        }
    }

    let config = IniConfig::load_or_default();
    ConversionMatrix::get_targets_for_selection_with_config(&formats, &config)
}

/// Returns the context menu icon bitmap, created once and kept for the lifetime of
/// the DLL (Explorer does not take ownership of `hbmpItem`, so creating one per
/// menu invocation would leak a GDI object every time the menu opens).
pub fn get_app_icon_bitmap() -> windows::Win32::Graphics::Gdi::HBITMAP {
    use std::sync::OnceLock;
    static BITMAP: OnceLock<isize> = OnceLock::new();
    let handle = *BITMAP.get_or_init(|| unsafe { create_app_icon_bitmap().0 as isize });
    windows::Win32::Graphics::Gdi::HBITMAP(handle as *mut _)
}

/// Loads the application icon and converts it into a 32-bit PARGB HBITMAP.
unsafe fn create_app_icon_bitmap() -> windows::Win32::Graphics::Gdi::HBITMAP {
    use windows::core::PCWSTR;
    use windows::Win32::Graphics::Gdi::{
        CreateCompatibleDC, CreateDIBSection, DeleteDC, GetDC, ReleaseDC, SelectObject, BITMAPINFO,
        BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        DestroyIcon, DrawIconEx, GetSystemMetrics, LoadImageW, DI_NORMAL, HICON, IMAGE_ICON,
        LR_DEFAULTCOLOR, LR_LOADFROMFILE, SM_CXSMICON, SM_CYSMICON,
    };

    let cx = GetSystemMetrics(SM_CXSMICON);
    let cy = GetSystemMetrics(SM_CYSMICON);
    let cx = if cx > 0 { cx } else { 16 };
    let cy = if cy > 0 { cy } else { 16 };

    let mut hicon = HICON::default();

    // 1. Primary: Load embedded icon resource #1 from our own DLL module instance
    let hinst = crate::get_instance_handle();
    if !hinst.0.is_null() {
        #[allow(clippy::manual_dangling_ptr)]
        if let Ok(handle) = LoadImageW(
            hinst,
            PCWSTR(1 as *const u16),
            IMAGE_ICON,
            cx,
            cy,
            LR_DEFAULTCOLOR,
        ) {
            if !handle.0.is_null() {
                hicon = HICON(handle.0);
            }
        }
    }

    // 2. Fallback: app.ico shipped next to the DLL
    if hicon.0.is_null() {
        if let Some(dir) = crate::module_dir() {
            let ico_path = dir.join("app.ico");
            let wide: Vec<u16> = ico_path
                .to_string_lossy()
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            if let Ok(handle) = LoadImageW(
                None,
                PCWSTR(wide.as_ptr()),
                IMAGE_ICON,
                cx,
                cy,
                LR_LOADFROMFILE,
            ) {
                if !handle.0.is_null() {
                    hicon = HICON(handle.0);
                }
            }
        }
    }

    if hicon.0.is_null() {
        return HBITMAP::default();
    }

    let hdc_screen = GetDC(None);
    let hdc_mem = CreateCompatibleDC(hdc_screen);

    let bi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: cx,
            biHeight: cy, // Standard bottom-up DIB
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: (cx * cy * 4) as u32,
            ..Default::default()
        },
        ..Default::default()
    };

    let mut ppv_bits: *mut std::ffi::c_void = std::ptr::null_mut();
    let hbmp = CreateDIBSection(hdc_mem, &bi, DIB_RGB_COLORS, &mut ppv_bits, None, 0);

    let mut result_hbmp = HBITMAP::default();

    if let Ok(hbmp) = hbmp {
        if !hbmp.0.is_null() && !ppv_bits.is_null() {
            let old_obj = SelectObject(hdc_mem, hbmp);
            let _ = DrawIconEx(hdc_mem, 0, 0, hicon, cx, cy, 0, None, DI_NORMAL);
            SelectObject(hdc_mem, old_obj);

            // Ensure 32-bit PARGB menu bitmap alpha channel is populated
            let pixel_count = (cx * cy) as usize;
            let pixels = std::slice::from_raw_parts_mut(ppv_bits as *mut u32, pixel_count);
            let has_any_alpha = pixels.iter().any(|&p| (p >> 24) > 0);
            if !has_any_alpha {
                for p in pixels.iter_mut() {
                    if *p & 0x00FFFFFF != 0 {
                        *p |= 0xFF000000;
                    }
                }
            }

            result_hbmp = hbmp;
        }
    }

    let _ = DeleteDC(hdc_mem);
    let _ = ReleaseDC(None, hdc_screen);
    let _ = DestroyIcon(hicon);

    result_hbmp
}
