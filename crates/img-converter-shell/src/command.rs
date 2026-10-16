use crate::enum_commands::create_enum_explorer_command;
use crate::process::spawn_converter;
use crate::utils::{
    alloc_co_task_str, get_app_icon_bitmap, get_available_targets_for_files,
    get_paths_from_item_array,
};
use img_converter_common::ImageFormat;
use std::ffi::c_void;
use std::mem::ManuallyDrop;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Mutex;
use windows::Win32::Foundation::{
    BOOL, E_FAIL, E_INVALIDARG, E_NOINTERFACE, E_NOTIMPL, E_POINTER, LPARAM, LRESULT, S_OK, WPARAM,
};
use windows::Win32::System::Com::{
    IDataObject, DVASPECT_CONTENT, FORMATETC, STGMEDIUM, TYMED_HGLOBAL,
};
use windows::Win32::System::Memory::{GlobalLock, GlobalUnlock};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    DragQueryFileW, IContextMenu, IContextMenu2, IContextMenu2_Vtbl, IContextMenu3,
    IContextMenu3_Vtbl, IContextMenu_Vtbl, IEnumExplorerCommand, IExplorerCommand,
    IExplorerCommand_Vtbl, IShellExtInit, IShellExtInit_Vtbl, IShellItemArray, CMF_DEFAULTONLY,
    CMINVOKECOMMANDINFO, ECF_DEFAULT, ECF_HASSUBCOMMANDS, ECS_ENABLED, ECS_HIDDEN, HDROP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreatePopupMenu, InsertMenuItemW, HMENU, MENUITEMINFOW, MFT_SEPARATOR, MIIM_BITMAP, MIIM_FTYPE,
    MIIM_ID, MIIM_STRING, MIIM_SUBMENU,
};
use windows_core::{IUnknown, Interface, GUID, HRESULT, PSTR, PWSTR};

#[link(name = "ole32")]
extern "system" {
    fn ReleaseStgMedium(pmedium: *mut STGMEDIUM);
}

const CF_HDROP_ID: u16 = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubCommandTarget {
    Format(ImageFormat),
    Configure,
}

#[repr(C)]
pub struct UnifiedShellExtension {
    pub vtbl_explorer_command: *const IExplorerCommand_Vtbl,
    pub vtbl_shell_ext_init: *const IShellExtInit_Vtbl,
    pub vtbl_context_menu: *const IContextMenu3_Vtbl,
    pub ref_count: AtomicU32,
    pub target: Option<SubCommandTarget>,
    pub selected_files: Mutex<Vec<PathBuf>>,
    pub sub_commands: Mutex<Vec<ImageFormat>>,
}

// -----------------------------------------------------------------------------
// Vtables
// -----------------------------------------------------------------------------

static EXPLORER_COMMAND_VTBL: IExplorerCommand_Vtbl = IExplorerCommand_Vtbl {
    base__: windows_core::IUnknown_Vtbl {
        QueryInterface: explorer_cmd_query_interface,
        AddRef: explorer_cmd_add_ref,
        Release: explorer_cmd_release,
    },
    GetTitle: cmd_get_title,
    GetIcon: cmd_get_icon,
    GetToolTip: cmd_get_tooltip,
    GetCanonicalName: cmd_get_canonical_name,
    GetState: cmd_get_state,
    Invoke: cmd_invoke,
    GetFlags: cmd_get_flags,
    EnumSubCommands: cmd_enum_sub_commands,
};

static SHELL_EXT_INIT_VTBL: IShellExtInit_Vtbl = IShellExtInit_Vtbl {
    base__: windows_core::IUnknown_Vtbl {
        QueryInterface: shell_ext_init_query_interface,
        AddRef: shell_ext_init_add_ref,
        Release: shell_ext_init_release,
    },
    Initialize: shell_ext_initialize,
};

static CONTEXT_MENU_VTBL: IContextMenu3_Vtbl = IContextMenu3_Vtbl {
    base__: IContextMenu2_Vtbl {
        base__: IContextMenu_Vtbl {
            base__: windows_core::IUnknown_Vtbl {
                QueryInterface: context_menu_query_interface,
                AddRef: context_menu_add_ref,
                Release: context_menu_release,
            },
            QueryContextMenu: context_menu_query_context_menu,
            InvokeCommand: context_menu_invoke_command,
            GetCommandString: context_menu_get_command_string,
        },
        HandleMenuMsg: context_menu_handle_menu_msg,
    },
    HandleMenuMsg2: context_menu_handle_menu_msg2,
};

static GLOBAL_SELECTED_FILES: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

// -----------------------------------------------------------------------------
// Construction & Global QueryInterface
// -----------------------------------------------------------------------------

impl UnifiedShellExtension {
    pub fn new(
        target: Option<SubCommandTarget>,
        selected_files: Vec<PathBuf>,
    ) -> *mut UnifiedShellExtension {
        crate::inc_object_count();
        let obj = Box::new(UnifiedShellExtension {
            vtbl_explorer_command: &EXPLORER_COMMAND_VTBL,
            vtbl_shell_ext_init: &SHELL_EXT_INIT_VTBL,
            vtbl_context_menu: &CONTEXT_MENU_VTBL,
            ref_count: AtomicU32::new(1),
            target,
            selected_files: Mutex::new(selected_files),
            sub_commands: Mutex::new(Vec::new()),
        });
        Box::into_raw(obj)
    }
}

pub unsafe fn create_unified_extension(
    target: Option<SubCommandTarget>,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    create_unified_extension_with_files(target, &[], riid, ppv)
}

pub unsafe fn create_unified_extension_with_files(
    target: Option<SubCommandTarget>,
    files: &[PathBuf],
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    let obj = UnifiedShellExtension::new(target, files.to_vec());
    let hr = unified_query_interface(obj, riid, ppv);
    unified_release(obj);
    hr
}

pub fn open_config_file() {
    let config_path = img_converter_common::IniConfig::config_path();
    if !config_path.exists() {
        let _ = img_converter_common::IniConfig::load_or_default();
    }
    let _ = Command::new("notepad.exe").arg(&config_path).spawn();
}

unsafe fn unified_query_interface(
    this: *mut UnifiedShellExtension,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() || riid.is_null() {
        return E_POINTER;
    }
    *ppv = std::ptr::null_mut();

    let iid = *riid;

    // Only interfaces whose vtable layout is actually implemented may be returned.
    if iid == IUnknown::IID || iid == IExplorerCommand::IID {
        (*this).ref_count.fetch_add(1, Ordering::Relaxed);
        *ppv = &mut (*this).vtbl_explorer_command as *mut _ as *mut c_void;
        S_OK
    } else if iid == IShellExtInit::IID {
        (*this).ref_count.fetch_add(1, Ordering::Relaxed);
        *ppv = &mut (*this).vtbl_shell_ext_init as *mut _ as *mut c_void;
        S_OK
    } else if iid == IContextMenu::IID || iid == IContextMenu2::IID || iid == IContextMenu3::IID {
        (*this).ref_count.fetch_add(1, Ordering::Relaxed);
        *ppv = &mut (*this).vtbl_context_menu as *mut _ as *mut c_void;
        S_OK
    } else {
        E_NOINTERFACE
    }
}

unsafe fn unified_add_ref(this: *mut UnifiedShellExtension) -> u32 {
    (*this).ref_count.fetch_add(1, Ordering::Relaxed) + 1
}

unsafe fn unified_release(this: *mut UnifiedShellExtension) -> u32 {
    let count = (*this).ref_count.fetch_sub(1, Ordering::Relaxed) - 1;
    if count == 0 {
        crate::dec_object_count();
        drop(Box::from_raw(this));
    }
    count
}

// -----------------------------------------------------------------------------
// Vtable Thunks (ExplorerCommand)
// -----------------------------------------------------------------------------

unsafe extern "system" fn explorer_cmd_query_interface(
    this: *mut c_void,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    // The IExplorerCommand vtable is the first field, so `this` is the object itself.
    unified_query_interface(this as *mut UnifiedShellExtension, riid, ppv)
}

unsafe extern "system" fn explorer_cmd_add_ref(this: *mut c_void) -> u32 {
    let obj = this as *mut UnifiedShellExtension;
    unified_add_ref(obj)
}

unsafe extern "system" fn explorer_cmd_release(this: *mut c_void) -> u32 {
    let obj = this as *mut UnifiedShellExtension;
    unified_release(obj)
}

// -----------------------------------------------------------------------------
// Vtable Thunks (ShellExtInit)
// -----------------------------------------------------------------------------

unsafe extern "system" fn shell_ext_init_query_interface(
    this: *mut c_void,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    let offset = std::mem::offset_of!(UnifiedShellExtension, vtbl_shell_ext_init);
    let obj = (this as *mut u8).sub(offset) as *mut UnifiedShellExtension;
    unified_query_interface(obj, riid, ppv)
}

unsafe extern "system" fn shell_ext_init_add_ref(this: *mut c_void) -> u32 {
    let offset = std::mem::offset_of!(UnifiedShellExtension, vtbl_shell_ext_init);
    let obj = (this as *mut u8).sub(offset) as *mut UnifiedShellExtension;
    unified_add_ref(obj)
}

unsafe extern "system" fn shell_ext_init_release(this: *mut c_void) -> u32 {
    let offset = std::mem::offset_of!(UnifiedShellExtension, vtbl_shell_ext_init);
    let obj = (this as *mut u8).sub(offset) as *mut UnifiedShellExtension;
    unified_release(obj)
}

// -----------------------------------------------------------------------------
// Vtable Thunks (ContextMenu)
// -----------------------------------------------------------------------------

unsafe extern "system" fn context_menu_query_interface(
    this: *mut c_void,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    let offset = std::mem::offset_of!(UnifiedShellExtension, vtbl_context_menu);
    let obj = (this as *mut u8).sub(offset) as *mut UnifiedShellExtension;
    unified_query_interface(obj, riid, ppv)
}

unsafe extern "system" fn context_menu_add_ref(this: *mut c_void) -> u32 {
    let offset = std::mem::offset_of!(UnifiedShellExtension, vtbl_context_menu);
    let obj = (this as *mut u8).sub(offset) as *mut UnifiedShellExtension;
    unified_add_ref(obj)
}

unsafe extern "system" fn context_menu_release(this: *mut c_void) -> u32 {
    let offset = std::mem::offset_of!(UnifiedShellExtension, vtbl_context_menu);
    let obj = (this as *mut u8).sub(offset) as *mut UnifiedShellExtension;
    unified_release(obj)
}

// -----------------------------------------------------------------------------
// IExplorerCommand Implementation
// -----------------------------------------------------------------------------

unsafe extern "system" fn cmd_get_title(
    this: *mut c_void,
    psi_item_array: *mut c_void,
    ppsz_name: *mut PWSTR,
) -> HRESULT {
    if ppsz_name.is_null() {
        return E_POINTER;
    }
    let config = img_converter_common::IniConfig::load_or_default();
    let lang = config.resolved_language();

    let obj = this as *mut UnifiedShellExtension;
    let title_str = match (*obj).target {
        None => {
            let mut count = 0usize;
            if !psi_item_array.is_null() {
                let item_array = ManuallyDrop::new(std::mem::transmute_copy::<
                    *mut c_void,
                    IShellItemArray,
                >(&psi_item_array));
                let paths = get_paths_from_item_array(Some(&item_array));
                if !paths.is_empty() {
                    count = paths.len();
                    if let Ok(mut guard) = (*obj).selected_files.lock() {
                        *guard = paths;
                    }
                }
            }
            if count == 0 {
                if let Ok(guard) = (*obj).selected_files.lock() {
                    count = guard.len();
                }
            }
            lang.convert_title(count)
        }
        Some(SubCommandTarget::Format(fmt)) => fmt.display_name().to_string(),
        Some(SubCommandTarget::Configure) => lang.configure_title().to_string(),
    };
    match alloc_co_task_str(&title_str) {
        Ok(pwstr) => {
            *ppsz_name = pwstr;
            S_OK
        }
        Err(hr) => hr,
    }
}

unsafe extern "system" fn cmd_get_icon(
    this: *mut c_void,
    _psi_item_array: *mut c_void,
    ppsz_icon: *mut PWSTR,
) -> HRESULT {
    if ppsz_icon.is_null() {
        return E_POINTER;
    }
    let obj = this as *mut UnifiedShellExtension;
    if (*obj).target.is_none() {
        let dir = crate::module_dir().unwrap_or_else(crate::default_install_dir);
        let ico_path = dir.join("app.ico");
        let icon = if ico_path.exists() {
            ico_path.display().to_string()
        } else {
            format!("{},0", dir.join("img-converter.exe").display())
        };
        if let Ok(pwstr) = alloc_co_task_str(&icon) {
            *ppsz_icon = pwstr;
            return S_OK;
        }
    }
    *ppsz_icon = PWSTR(std::ptr::null_mut());
    E_NOTIMPL
}

unsafe extern "system" fn cmd_get_tooltip(
    this: *mut c_void,
    _psi_item_array: *mut c_void,
    ppsz_infotip: *mut PWSTR,
) -> HRESULT {
    if ppsz_infotip.is_null() {
        return E_POINTER;
    }
    let config = img_converter_common::IniConfig::load_or_default();
    let lang = config.resolved_language();

    let obj = this as *mut UnifiedShellExtension;
    let tip = match (*obj).target {
        None => lang.root_tooltip(),
        Some(SubCommandTarget::Format(fmt)) => fmt.display_name(),
        Some(SubCommandTarget::Configure) => lang.configure_tooltip(),
    };
    match alloc_co_task_str(tip) {
        Ok(pwstr) => {
            *ppsz_infotip = pwstr;
            S_OK
        }
        Err(hr) => hr,
    }
}

unsafe extern "system" fn cmd_get_canonical_name(
    _this: *mut c_void,
    pguid_command_name: *mut GUID,
) -> HRESULT {
    if pguid_command_name.is_null() {
        return E_POINTER;
    }
    *pguid_command_name = crate::CLSID_IMAGE_CONVERTER;
    S_OK
}

unsafe extern "system" fn cmd_get_state(
    this: *mut c_void,
    psi_item_array: *mut c_void,
    _f_ok_to_be_slow: BOOL,
    pdw_cmd_state: *mut u32,
) -> HRESULT {
    if pdw_cmd_state.is_null() {
        return E_POINTER;
    }

    let obj = this as *mut UnifiedShellExtension;
    let items: Option<ManuallyDrop<IShellItemArray>> = if !psi_item_array.is_null() {
        Some(ManuallyDrop::new(std::mem::transmute_copy(&psi_item_array)))
    } else {
        None
    };

    let item_array_ref = items.as_deref();
    let mut paths = get_paths_from_item_array(item_array_ref);
    if !paths.is_empty() {
        if let Ok(mut guard) = (*obj).selected_files.lock() {
            *guard = paths.clone();
        }
        if let Ok(mut global) = GLOBAL_SELECTED_FILES.lock() {
            *global = paths.clone();
        }
    } else {
        if let Ok(guard) = (*obj).selected_files.lock() {
            paths = guard.clone();
        }
        if paths.is_empty() {
            if let Ok(guard) = GLOBAL_SELECTED_FILES.lock() {
                paths = guard.clone();
            }
        }
    }
    let available = get_available_targets_for_files(&paths);

    match (*obj).target {
        None => {
            if available.is_empty() {
                *pdw_cmd_state = ECS_HIDDEN.0 as u32;
            } else {
                *pdw_cmd_state = ECS_ENABLED.0 as u32;
            }
        }
        Some(SubCommandTarget::Format(fmt)) => {
            if available.contains(&fmt) {
                *pdw_cmd_state = ECS_ENABLED.0 as u32;
            } else {
                *pdw_cmd_state = ECS_HIDDEN.0 as u32;
            }
        }
        Some(SubCommandTarget::Configure) => {
            *pdw_cmd_state = ECS_ENABLED.0 as u32;
        }
    }
    S_OK
}

unsafe extern "system" fn cmd_invoke(
    this: *mut c_void,
    psi_item_array: *mut c_void,
    _pbc: *mut c_void,
) -> HRESULT {
    let obj = this as *mut UnifiedShellExtension;
    let Some(target) = (*obj).target else {
        return S_OK;
    };

    match target {
        SubCommandTarget::Configure => {
            open_config_file();
            S_OK
        }
        SubCommandTarget::Format(fmt) => {
            let items: Option<ManuallyDrop<IShellItemArray>> = if !psi_item_array.is_null() {
                Some(ManuallyDrop::new(std::mem::transmute_copy(&psi_item_array)))
            } else {
                None
            };

            let item_array_ref = items.as_deref();
            let mut paths = get_paths_from_item_array(item_array_ref);
            // Never fall back to the process-wide selection here: it may belong to
            // another Explorer window and would convert the wrong files.
            if paths.is_empty() {
                if let Ok(guard) = (*obj).selected_files.lock() {
                    paths = guard.clone();
                }
            }
            if paths.is_empty() {
                return S_OK;
            }

            match spawn_converter(&paths, fmt) {
                Ok(_) => S_OK,
                Err(e) => e.code(),
            }
        }
    }
}

unsafe extern "system" fn cmd_get_flags(this: *mut c_void, pdw_flags: *mut u32) -> HRESULT {
    if pdw_flags.is_null() {
        return E_POINTER;
    }
    let obj = this as *mut UnifiedShellExtension;
    if (*obj).target.is_none() {
        *pdw_flags = ECF_HASSUBCOMMANDS.0 as u32;
    } else {
        *pdw_flags = ECF_DEFAULT.0 as u32;
    }
    S_OK
}

unsafe extern "system" fn cmd_enum_sub_commands(
    this: *mut c_void,
    pp_enum: *mut *mut c_void,
) -> HRESULT {
    if pp_enum.is_null() {
        return E_POINTER;
    }
    *pp_enum = std::ptr::null_mut();

    let obj = this as *mut UnifiedShellExtension;
    if (*obj).target.is_some() {
        return E_NOTIMPL;
    }

    let config = img_converter_common::IniConfig::load_or_default();
    let mut all_outputs: Vec<SubCommandTarget> = ImageFormat::OUTPUT_FORMATS
        .iter()
        .filter(|&&fmt| config.is_format_enabled(fmt))
        .map(|&fmt| SubCommandTarget::Format(fmt))
        .collect();
    all_outputs.push(SubCommandTarget::Configure);

    let parent_files = (*obj)
        .selected_files
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default();
    let files_to_pass = if !parent_files.is_empty() {
        parent_files
    } else {
        GLOBAL_SELECTED_FILES
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default()
    };

    create_enum_explorer_command(
        all_outputs,
        files_to_pass,
        &IEnumExplorerCommand::IID,
        pp_enum,
    )
}

// -----------------------------------------------------------------------------
// IShellExtInit & IContextMenu Implementation (Classic Context Menu)
// -----------------------------------------------------------------------------

unsafe extern "system" fn shell_ext_initialize(
    this: *mut c_void,
    _pidl_folder: *const ITEMIDLIST,
    pdtobj: *mut c_void,
    _hkey_prog_id: windows::Win32::System::Registry::HKEY,
) -> HRESULT {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if pdtobj.is_null() {
            return E_INVALIDARG;
        }

        let offset = std::mem::offset_of!(UnifiedShellExtension, vtbl_shell_ext_init);
        let obj = (this as *mut u8).sub(offset) as *mut UnifiedShellExtension;

        // Use ManuallyDrop to avoid decrementing the caller's IDataObject refcount!
        let data_obj = ManuallyDrop::new(std::mem::transmute_copy::<*mut c_void, IDataObject>(
            &pdtobj,
        ));

        let format_etc = FORMATETC {
            cfFormat: CF_HDROP_ID,
            ptd: std::ptr::null_mut(),
            dwAspect: DVASPECT_CONTENT.0,
            lindex: -1,
            tymed: TYMED_HGLOBAL.0 as u32,
        };

        let mut medium = match data_obj.GetData(&format_etc) {
            Ok(m) => m,
            Err(_) => return E_FAIL,
        };

        let hglobal = windows::Win32::Foundation::HGLOBAL(medium.u.hGlobal.0);
        let lock_ptr = GlobalLock(hglobal);
        if lock_ptr.is_null() {
            ReleaseStgMedium(&mut medium);
            return E_FAIL;
        }

        let hdrop = HDROP(lock_ptr);
        let file_count = DragQueryFileW(hdrop, 0xFFFFFFFF, None);
        let mut paths = Vec::with_capacity(file_count as usize);

        for i in 0..file_count {
            let len = DragQueryFileW(hdrop, i, None);
            if len > 0 {
                let mut buffer = vec![0u16; (len + 1) as usize];
                DragQueryFileW(hdrop, i, Some(&mut buffer));
                let path_str = String::from_utf16_lossy(&buffer[..len as usize]);
                paths.push(PathBuf::from(path_str));
            }
        }

        let _ = GlobalUnlock(hglobal);
        ReleaseStgMedium(&mut medium);

        if let Ok(mut files_guard) = (*obj).selected_files.lock() {
            *files_guard = paths;
        }

        S_OK
    }));

    result.unwrap_or(E_FAIL)
}

unsafe extern "system" fn context_menu_query_context_menu(
    this: *mut c_void,
    hmenu: HMENU,
    index_menu: u32,
    id_cmd_first: u32,
    _id_cmd_last: u32,
    u_flags: u32,
) -> HRESULT {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if (u_flags & CMF_DEFAULTONLY) != 0 {
            return S_OK;
        }

        let offset = std::mem::offset_of!(UnifiedShellExtension, vtbl_context_menu);
        let obj = (this as *mut u8).sub(offset) as *mut UnifiedShellExtension;

        let files = (*obj)
            .selected_files
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();
        let targets = get_available_targets_for_files(&files);
        if targets.is_empty() {
            return S_OK;
        }

        if let Ok(mut subs) = (*obj).sub_commands.lock() {
            *subs = targets.clone();
        }

        let h_sub_menu = match CreatePopupMenu() {
            Ok(m) => m,
            Err(_) => return E_FAIL,
        };

        // 1. Format conversion options (PNG, JPEG, WEBP, AVIF)
        for (idx, target) in targets.iter().enumerate() {
            let name = target.display_name();
            let mut wide_name: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();

            let item = MENUITEMINFOW {
                cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
                fMask: MIIM_STRING | MIIM_ID,
                wID: id_cmd_first + (idx as u32),
                dwTypeData: PWSTR(wide_name.as_mut_ptr()),
                ..Default::default()
            };

            let _ = InsertMenuItemW(h_sub_menu, idx as u32, true, &item);
        }

        // 2. Menu separator
        let sep_item = MENUITEMINFOW {
            cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
            fMask: MIIM_FTYPE,
            fType: MFT_SEPARATOR,
            ..Default::default()
        };
        let _ = InsertMenuItemW(h_sub_menu, targets.len() as u32, true, &sep_item);

        // 3. Configure option
        let config = img_converter_common::IniConfig::load_or_default();
        let lang = config.resolved_language();

        let mut conf_title: Vec<u16> = lang
            .configure_title()
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let conf_item = MENUITEMINFOW {
            cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
            fMask: MIIM_STRING | MIIM_ID,
            wID: id_cmd_first + (targets.len() as u32),
            dwTypeData: PWSTR(conf_title.as_mut_ptr()),
            ..Default::default()
        };
        let _ = InsertMenuItemW(h_sub_menu, (targets.len() + 1) as u32, true, &conf_item);

        let root_title_str = lang.convert_title(files.len());
        let mut root_title: Vec<u16> = root_title_str
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let icon_hbmp = get_app_icon_bitmap();
        // The submenu parent needs no command ID (assigning one would collide with
        // the first format item).
        let mut fmask = MIIM_STRING | MIIM_SUBMENU;
        if !icon_hbmp.0.is_null() {
            fmask |= MIIM_BITMAP;
        }

        let root_item = MENUITEMINFOW {
            cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
            fMask: fmask,
            hSubMenu: h_sub_menu,
            hbmpItem: icon_hbmp,
            dwTypeData: PWSTR(root_title.as_mut_ptr()),
            ..Default::default()
        };

        let _ = InsertMenuItemW(hmenu, index_menu, true, &root_item);

        // We reserved (targets.len() + 1) IDs (formats + configure)
        windows_core::HRESULT(((targets.len() + 1) as i32) & 0xFFFF)
    }));

    result.unwrap_or(E_FAIL)
}

unsafe extern "system" fn context_menu_invoke_command(
    this: *mut c_void,
    pici: *const CMINVOKECOMMANDINFO,
) -> HRESULT {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if pici.is_null() {
            return E_INVALIDARG;
        }

        let offset = std::mem::offset_of!(UnifiedShellExtension, vtbl_context_menu);
        let obj = (this as *mut u8).sub(offset) as *mut UnifiedShellExtension;

        let verb = (*pici).lpVerb.0 as usize;
        if verb > 0xFFFF {
            return E_NOTIMPL;
        }

        let item_offset = verb;
        let subs = (*obj)
            .sub_commands
            .lock()
            .map(|g| g.clone())
            .unwrap_or_default();

        if item_offset == subs.len() {
            // User clicked "Configure..."
            open_config_file();
            return S_OK;
        }

        if item_offset < subs.len() {
            let target = subs[item_offset];
            let files = (*obj)
                .selected_files
                .lock()
                .map(|g| g.clone())
                .unwrap_or_default();
            match spawn_converter(&files, target) {
                Ok(_) => S_OK,
                Err(e) => e.code(),
            }
        } else {
            E_FAIL
        }
    }));

    result.unwrap_or(E_FAIL)
}

unsafe extern "system" fn context_menu_get_command_string(
    _this: *mut c_void,
    _id_cmd: usize,
    _u_type: u32,
    _p_reserved: *const u32,
    _psz_name: PSTR,
    _cch_max: u32,
) -> HRESULT {
    E_NOTIMPL
}

unsafe extern "system" fn context_menu_handle_menu_msg(
    _this: *mut c_void,
    _u_msg: u32,
    _w_param: WPARAM,
    _l_param: LPARAM,
) -> HRESULT {
    S_OK
}

unsafe extern "system" fn context_menu_handle_menu_msg2(
    _this: *mut c_void,
    _u_msg: u32,
    _w_param: WPARAM,
    _l_param: LPARAM,
    _pl_result: *mut LRESULT,
) -> HRESULT {
    S_OK
}
