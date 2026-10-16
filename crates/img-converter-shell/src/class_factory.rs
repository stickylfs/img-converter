use crate::command::create_unified_extension;
use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};
use windows::Win32::Foundation::{BOOL, CLASS_E_NOAGGREGATION, E_NOINTERFACE, E_POINTER, S_OK};
use windows::Win32::System::Com::{IClassFactory, IClassFactory_Vtbl};
use windows_core::{IUnknown, Interface, GUID, HRESULT};

#[repr(C)]
pub struct ClassFactory {
    pub vtbl: *const IClassFactory_Vtbl,
    pub ref_count: AtomicU32,
}

static CLASS_FACTORY_VTBL: IClassFactory_Vtbl = IClassFactory_Vtbl {
    base__: windows_core::IUnknown_Vtbl {
        QueryInterface: class_factory_query_interface,
        AddRef: class_factory_add_ref,
        Release: class_factory_release,
    },
    CreateInstance: class_factory_create_instance,
    LockServer: class_factory_lock_server,
};

impl ClassFactory {
    pub fn new() -> *mut ClassFactory {
        crate::inc_object_count();
        let obj = Box::new(ClassFactory {
            vtbl: &CLASS_FACTORY_VTBL,
            ref_count: AtomicU32::new(1),
        });
        Box::into_raw(obj)
    }
}

pub unsafe fn create_class_factory(riid: *const GUID, ppv: *mut *mut c_void) -> HRESULT {
    let factory = ClassFactory::new();
    let hr = class_factory_query_interface(factory as *mut c_void, riid, ppv);
    class_factory_release(factory as *mut c_void);
    hr
}

unsafe extern "system" fn class_factory_query_interface(
    this: *mut c_void,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() || riid.is_null() {
        return E_POINTER;
    }
    *ppv = std::ptr::null_mut();

    let iid = *riid;
    if iid == IUnknown::IID || iid == IClassFactory::IID {
        let factory = this as *mut ClassFactory;
        (*factory).ref_count.fetch_add(1, Ordering::Relaxed);
        *ppv = this;
        S_OK
    } else {
        E_NOINTERFACE
    }
}

unsafe extern "system" fn class_factory_add_ref(this: *mut c_void) -> u32 {
    let factory = this as *mut ClassFactory;
    (*factory).ref_count.fetch_add(1, Ordering::Relaxed) + 1
}

unsafe extern "system" fn class_factory_release(this: *mut c_void) -> u32 {
    let factory = this as *mut ClassFactory;
    let count = (*factory).ref_count.fetch_sub(1, Ordering::Relaxed) - 1;
    if count == 0 {
        crate::dec_object_count();
        drop(Box::from_raw(factory));
    }
    count
}

unsafe extern "system" fn class_factory_create_instance(
    _this: *mut c_void,
    punk_outer: *mut c_void,
    riid: *const GUID,
    ppv: *mut *mut c_void,
) -> HRESULT {
    if ppv.is_null() || riid.is_null() {
        return E_POINTER;
    }
    *ppv = std::ptr::null_mut();

    if !punk_outer.is_null() {
        return CLASS_E_NOAGGREGATION;
    }

    create_unified_extension(None, riid, ppv)
}

unsafe extern "system" fn class_factory_lock_server(_this: *mut c_void, flock: BOOL) -> HRESULT {
    if flock.as_bool() {
        crate::inc_object_count();
    } else {
        crate::dec_object_count();
    }
    S_OK
}
