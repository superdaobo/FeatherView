//! LanxiaPreviewHandler —— 览匣 FeatherView 的 Windows 资源管理器预览处理程序（进程内 COM 服务器）。
//!
//! 架构文档：docs/architecture/windows-preview-handler.md
//! CLSID：{8895b1c6-b41f-4c1c-a562-0d564250836f}
//! 实现接口：IPreviewHandler / IObjectWithSite / IOleWindow / IInitializeWithStream
//!
//! 本 crate 编译为 cdylib（lanxia_preview_handler.dll），导出经典 COM 入口：
//! - DllGetClassObject：按 CLSID 返回类工厂
//! - DllCanUnloadNow：活动对象计数为 0 时允许卸载

mod class_factory;
mod preview_handler;

use std::sync::atomic::{AtomicI32, Ordering};

use windows::core::{GUID, HRESULT, IUnknown, IUnknown_Vtbl};
use windows::Win32::Foundation::{S_FALSE, S_OK};

/// 预览处理器 CLSID（固定值，注册表/文档引用）
/// 预览处理器 CLSID（固定值：8895b1c6-b41f-4c1c-a562-0d564250836f）
pub const CLSID_PREVIEW_HANDLER: GUID = GUID::from_u128(0x8895b1c6_b41f_4c1c_a562_0d564250836f);

/// 服务器级活动计数（类工厂 LockServer + 活动对象）
static ACTIVE: AtomicI32 = AtomicI32::new(0);

pub(crate) fn lock_server(lock: bool) {
    if lock {
        ACTIVE.fetch_add(1, Ordering::SeqCst);
    } else {
        ACTIVE.fetch_sub(1, Ordering::SeqCst);
    }
}

/// 将预览字节解码为 UTF-8 文本（复用 featherview-core 的编码检测）。
fn decode_preview(bytes: &[u8]) -> String {
    match featherview_core::decode_to_utf8(bytes) {
        Ok((text, _)) => text,
        Err(_) => "<无法解码此文件内容>".to_string(),
    }
}

/// 活动对象计数（DllCanUnloadNow 用）
fn can_unload_now() -> bool {
    let total = ACTIVE.load(Ordering::SeqCst) + preview_handler::active_objects();
    total <= 0
}

#[no_mangle]
pub unsafe extern "system" fn DllGetClassObject(
    rclsid: *const GUID,
    riid: *const GUID,
    ppv: *mut *mut core::ffi::c_void,
) -> HRESULT {
    if rclsid.is_null() || riid.is_null() || ppv.is_null() {
        return HRESULT(0x80004003_u32 as _);
    }
    if *rclsid != CLSID_PREVIEW_HANDLER {
        return windows::Win32::Foundation::CLASS_E_CLASSNOTAVAILABLE;
    }
    match class_factory::create_factory(rclsid) {
        Ok(factory) => {
            // 直接操作工厂 vtable（避免依赖包装方法）
            let vtable = *(factory as *const *const IUnknown_Vtbl);
            let hr = ((*vtable).QueryInterface)(factory, riid, ppv);
            if hr != S_OK {
                ((*vtable).Release)(factory);
                *ppv = std::ptr::null_mut();
            }
            hr
        }
        Err(hr) => hr,
    }
}

#[no_mangle]
pub unsafe extern "system" fn DllCanUnloadNow() -> HRESULT {
    if can_unload_now() {
        S_OK
    } else {
        S_FALSE
    }
}
