//! IClassFactory 手动 COM 实现（windows-rs 0.58 无 _Impl trait，采用经典 vtable 布局）。
use std::sync::atomic::{AtomicI32, Ordering};

use windows::core::{GUID, HRESULT, IUnknown, IUnknown_Vtbl};
use windows::Win32::Foundation::{CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_NOINTERFACE, S_OK};
use windows::Win32::System::Com::{IClassFactory_Vtbl};

use crate::preview_handler::PreviewHandlerObject;
use crate::CLSID_PREVIEW_HANDLER;

const E_POINTER: HRESULT = HRESULT(0x80004003_u32 as _);
const IID_IUNKNOWN: GUID = GUID::from_u128(0x00000000_0000_0000_c000_000000000046);
const IID_ICLASS_FACTORY: GUID = GUID::from_u128(0x00000001_0000_0000_c000_000000000046);

/// 类工厂对象：IClassFactory vtable + 引用计数
#[repr(C)]
pub struct FactoryObject {
    vtbl: *const IClassFactory_Vtbl,
    refcount: AtomicI32,
}

impl FactoryObject {
    pub fn new() -> *mut FactoryObject {
        let obj = Box::new(FactoryObject {
            vtbl: &FACTORY_VTABLE,
            refcount: AtomicI32::new(1),
        });
        Box::into_raw(obj)
    }

    unsafe fn from_this(this: *mut core::ffi::c_void) -> *mut FactoryObject {
        this as *mut FactoryObject
    }
}

unsafe extern "system" fn factory_query_interface(
    this: *mut core::ffi::c_void,
    riid: *const GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> HRESULT {
    if riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    let guid = &*riid;
    if guid == &IID_IUNKNOWN || guid == &IID_ICLASS_FACTORY {
        let obj = FactoryObject::from_this(this);
        (*obj).refcount.fetch_add(1, Ordering::SeqCst);
        *ppv = this;
        S_OK
    } else {
        E_NOINTERFACE
    }
}

unsafe extern "system" fn factory_add_ref(this: *mut core::ffi::c_void) -> u32 {
    let obj = FactoryObject::from_this(this);
    (*obj).refcount.fetch_add(1, Ordering::SeqCst) as u32 + 1
}

unsafe extern "system" fn factory_release(this: *mut core::ffi::c_void) -> u32 {
    let obj = FactoryObject::from_this(this);
    let remaining = (*obj).refcount.fetch_sub(1, Ordering::SeqCst) as u32 - 1;
    if remaining == 0 {
        drop(Box::from_raw(obj));
    }
    remaining
}

unsafe extern "system" fn factory_create_instance(
    _this: *mut core::ffi::c_void,
    punk_outer: *mut core::ffi::c_void,
    riid: *const GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> HRESULT {
    if !punk_outer.is_null() {
        return CLASS_E_NOAGGREGATION;
    }
    if riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    let obj = PreviewHandlerObject::new();
    let hr = unsafe { (*obj).query_interface(&*riid, ppv) };
    if hr == S_OK {
        // QI 已 AddRef（调用者持有）；释放 CreateInstance 的临时初始引用
        unsafe { (*obj).refcount.fetch_sub(1, Ordering::SeqCst) };
    } else {
        unsafe {
            drop(Box::from_raw(obj));
        }
    }
    hr
}

unsafe extern "system" fn factory_lock_server(
    _this: *mut core::ffi::c_void,
    flock: windows::Win32::Foundation::BOOL,
) -> HRESULT {
    if flock.0 != 0 {
        crate::lock_server(true);
    } else {
        crate::lock_server(false);
    }
    S_OK
}

static FACTORY_VTABLE: IClassFactory_Vtbl = IClassFactory_Vtbl {
    base__: IUnknown_Vtbl {
        QueryInterface: factory_query_interface,
        AddRef: factory_add_ref,
        Release: factory_release,
    },
    CreateInstance: factory_create_instance,
    LockServer: factory_lock_server,
};

/// DllGetClassObject 使用的类工厂创建函数
pub unsafe fn create_factory(rclsid: *const GUID) -> Result<*mut core::ffi::c_void, HRESULT> {
    if rclsid.is_null() {
        return Err(E_POINTER);
    }
    if *rclsid != CLSID_PREVIEW_HANDLER {
        return Err(CLASS_E_CLASSNOTAVAILABLE);
    }
    let factory = FactoryObject::new();
    Ok(factory as *mut core::ffi::c_void)
}
