use crate::command::{create_unified_extension_with_files, SubCommandTarget};
use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};
use windows::Win32::Foundation::{E_NOINTERFACE, E_POINTER, S_FALSE, S_OK};
use windows::Win32::UI::Shell::{
    IEnumExplorerCommand, IEnumExplorerCommand_Vtbl, IExplorerCommand,
};
use windows_core::{IUnknown, Interface, GUID, HRESULT};

#[repr(C)]
pub struct EnumExplorerCommand {
    pub vtbl: *const IEnumExplorerCommand_Vtbl,
    pub ref_count: AtomicU32,
    pub targets: Vec<SubCommandTarget>,
    pub selected_files: Vec<PathBuf>,
    pub current_index: AtomicUsize,
}

static ENUM_VTBL: IEnumExplorerCommand_Vtbl = IEnumExplorerCommand_Vtbl {
    base__: windows_core::IUnknown_Vtbl {
        QueryInterface: enum_query_interface,
        AddRef: enum_add_ref,
        Release: enum_release,
    },
    Next: enum_next,
    Skip: enum_skip,
    Reset: enum_reset,
    Clone: enum_clone,
};

impl EnumExplorerCommand {
    pub fn new(
        targets: Vec<SubCommandTarget>,
        selected_files: Vec<PathBuf>,
    ) -> *mut EnumExplorerCommand {
        crate::inc_object_count();
        let obj = Box::new(EnumExplorerCommand {
            vtbl: &ENUM_VTBL,
            ref_count: AtomicU32::new(1),
            targets,
            selected_files,
            current_index: AtomicUsize::new(0),
        });
        Box::into_raw(obj)
    }
}

pub unsafe fn create_enum_explorer_command(
    targets: Vec<SubCommandTarget>,
    selected_files: Vec<PathBuf>,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    let obj = EnumExplorerCommand::new(targets, selected_files);
    let hr = enum_query_interface(obj as *mut c_void, riid, ppv);
    enum_release(obj as *mut c_void);
    hr
}

unsafe extern "system" fn enum_query_interface(
    this: *mut c_void,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() || riid.is_null() {
        return E_POINTER;
    }
    *ppv = std::ptr::null_mut();

    let iid = *riid;
    if iid == IUnknown::IID || iid == IEnumExplorerCommand::IID {
        let obj = this as *mut EnumExplorerCommand;
        (*obj).ref_count.fetch_add(1, Ordering::Relaxed);
        *ppv = this;
        S_OK
    } else {
        E_NOINTERFACE
    }
}

unsafe extern "system" fn enum_add_ref(this: *mut c_void) -> u32 {
    let obj = this as *mut EnumExplorerCommand;
    (*obj).ref_count.fetch_add(1, Ordering::Relaxed) + 1
}

unsafe extern "system" fn enum_release(this: *mut c_void) -> u32 {
    let obj = this as *mut EnumExplorerCommand;
    let count = (*obj).ref_count.fetch_sub(1, Ordering::Relaxed) - 1;
    if count == 0 {
        crate::dec_object_count();
        drop(Box::from_raw(obj));
    }
    count
}

unsafe extern "system" fn enum_next(
    this: *mut c_void,
    celt: u32,
    p_ui_command: *mut *mut c_void,
    pcelt_fetched: *mut u32,
) -> HRESULT {
    if celt == 0 {
        return S_OK;
    }
    if p_ui_command.is_null() || (celt > 1 && pcelt_fetched.is_null()) {
        return E_POINTER;
    }

    let obj = this as *mut EnumExplorerCommand;
    let targets = &(*obj).targets;
    let mut fetched = 0;
    let mut idx = (*obj).current_index.load(Ordering::Relaxed);

    while fetched < celt && idx < targets.len() {
        let target = targets[idx];
        let mut sub_cmd_ptr = std::ptr::null_mut();
        let hr = create_unified_extension_with_files(
            Some(target),
            &(*obj).selected_files,
            &IExplorerCommand::IID,
            &mut sub_cmd_ptr,
        );
        if hr == S_OK {
            *p_ui_command.add(fetched as usize) = sub_cmd_ptr;
            fetched += 1;
        }
        idx += 1;
    }

    (*obj).current_index.store(idx, Ordering::Relaxed);

    if !pcelt_fetched.is_null() {
        *pcelt_fetched = fetched;
    }

    if fetched == celt {
        S_OK
    } else {
        S_FALSE
    }
}

unsafe extern "system" fn enum_skip(this: *mut c_void, celt: u32) -> HRESULT {
    let obj = this as *mut EnumExplorerCommand;
    let idx = (*obj).current_index.load(Ordering::Relaxed);
    let new_idx = (idx + celt as usize).min((*obj).targets.len());
    (*obj).current_index.store(new_idx, Ordering::Relaxed);
    if new_idx < (*obj).targets.len() {
        S_OK
    } else {
        S_FALSE
    }
}

unsafe extern "system" fn enum_reset(this: *mut c_void) -> HRESULT {
    let obj = this as *mut EnumExplorerCommand;
    (*obj).current_index.store(0, Ordering::Relaxed);
    S_OK
}

unsafe extern "system" fn enum_clone(this: *mut c_void, ppenum: *mut *mut c_void) -> HRESULT {
    if ppenum.is_null() {
        return E_POINTER;
    }
    *ppenum = std::ptr::null_mut();

    let obj = this as *mut EnumExplorerCommand;
    let clone_obj = EnumExplorerCommand::new((*obj).targets.clone(), (*obj).selected_files.clone());
    (*clone_obj).current_index.store(
        (*obj).current_index.load(Ordering::Relaxed),
        Ordering::Relaxed,
    );

    *ppenum = clone_obj as *mut c_void;
    S_OK
}
