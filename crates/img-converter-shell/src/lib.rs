#![allow(clippy::missing_safety_doc)]

pub mod class_factory;
pub mod command;
pub mod enum_commands;
pub mod process;
pub mod utils;

use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};
use windows::Win32::Foundation::{CLASS_E_CLASSNOTAVAILABLE, E_POINTER, HINSTANCE, S_OK};
use windows::Win32::System::SystemServices::DLL_PROCESS_ATTACH;
use windows_core::{GUID, HRESULT};

/// CLSID: {3CA619D1-34F6-42E1-B0E3-7AE990A32001}
pub const CLSID_IMAGE_CONVERTER: GUID = GUID::from_u128(0x3ca619d1_34f6_42e1_b0e3_7ae990a32001);

static OBJECT_COUNT: AtomicU32 = AtomicU32::new(0);
static G_HINSTANCE: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

pub fn get_instance_handle() -> HINSTANCE {
    HINSTANCE(G_HINSTANCE.load(Ordering::Relaxed) as *mut _)
}

/// Directory containing this DLL (the installation directory).
pub fn module_dir() -> Option<std::path::PathBuf> {
    use windows::Win32::System::LibraryLoader::GetModuleFileNameW;

    let hinst = get_instance_handle();
    if hinst.0.is_null() {
        return None;
    }
    let mut buf = vec![0u16; 32_768];
    let len = unsafe { GetModuleFileNameW(windows::Win32::Foundation::HMODULE(hinst.0), &mut buf) }
        as usize;
    if len == 0 || len >= buf.len() {
        return None;
    }
    let path = std::path::PathBuf::from(String::from_utf16_lossy(&buf[..len]));
    path.parent().map(std::path::Path::to_path_buf)
}

/// Default per-user installation directory (`%LOCALAPPDATA%\Programs\img-converter`).
pub fn default_install_dir() -> std::path::PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(std::path::PathBuf::from)
        .unwrap_or_default()
        .join("Programs")
        .join("img-converter")
}

pub fn inc_object_count() {
    OBJECT_COUNT.fetch_add(1, Ordering::Relaxed);
}

pub fn dec_object_count() {
    OBJECT_COUNT.fetch_sub(1, Ordering::Relaxed);
}

#[no_mangle]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() || rclsid.is_null() || riid.is_null() {
        return E_POINTER;
    }
    *ppv = std::ptr::null_mut();

    if *rclsid != CLSID_IMAGE_CONVERTER {
        return CLASS_E_CLASSNOTAVAILABLE;
    }

    class_factory::create_class_factory(riid, ppv)
}

#[no_mangle]
pub unsafe extern "system" fn DllCanUnloadNow() -> HRESULT {
    if OBJECT_COUNT.load(Ordering::Relaxed) == 0 {
        S_OK
    } else {
        windows::Win32::Foundation::S_FALSE
    }
}

#[no_mangle]
pub unsafe extern "system" fn DllRegisterServer() -> HRESULT {
    S_OK
}

#[no_mangle]
pub unsafe extern "system" fn DllUnregisterServer() -> HRESULT {
    S_OK
}

#[no_mangle]
pub unsafe extern "system" fn DllMain(hinst: isize, reason: u32, _reserved: *mut c_void) -> i32 {
    if reason == DLL_PROCESS_ATTACH {
        G_HINSTANCE.store(hinst, Ordering::SeqCst);
    }
    1
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::System::Com::IClassFactory;
    use windows::Win32::UI::Shell::{
        IContextMenu, IContextMenu3, IExplorerCommand, IExplorerCommandState, IShellExtInit,
    };
    use windows_core::Interface;

    #[test]
    fn test_com_objects_are_released() {
        unsafe {
            let mut ptr = std::ptr::null_mut();
            let hr = DllGetClassObject(&CLSID_IMAGE_CONVERTER, &IClassFactory::IID, &mut ptr);
            assert_eq!(hr, S_OK);
            let factory = IClassFactory::from_raw(ptr);

            let menu: IContextMenu = factory.CreateInstance(None).unwrap();
            // Every interface of the unified object must be reachable from the others.
            let command: IExplorerCommand = menu.cast().unwrap();
            let _init: IShellExtInit = command.cast().unwrap();
            let _menu3: IContextMenu3 = command.cast().unwrap();
            // Interfaces whose vtable is not implemented must be refused.
            assert!(command.cast::<IExplorerCommandState>().is_err());

            let unknown_clsid = GUID::from_u128(1);
            let mut other = std::ptr::null_mut();
            assert_eq!(
                DllGetClassObject(&unknown_clsid, &IClassFactory::IID, &mut other),
                CLASS_E_CLASSNOTAVAILABLE
            );

            assert_eq!(DllCanUnloadNow(), windows::Win32::Foundation::S_FALSE);
        }
        // All references dropped: the DLL can be unloaded again.
        assert_eq!(unsafe { DllCanUnloadNow() }, S_OK);
    }
}
