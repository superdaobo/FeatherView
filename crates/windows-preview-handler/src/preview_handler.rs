//! PreviewHandler 手动 COM 实现（windows-rs 0.58 无 _Impl trait，经典多接口 vtable 布局）。
use std::ffi::c_void;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Mutex, MutexGuard};

use windows::core::{w, GUID, HRESULT, Interface, IUnknown, IUnknown_Vtbl, PCWSTR};
use windows::Win32::Foundation::{BOOL, E_NOINTERFACE, HWND, RECT, S_OK};
use windows::Win32::System::Com::IStream;
use windows::Win32::System::Ole::IOleWindow_Vtbl;
use windows::Win32::UI::Shell::{IPreviewHandler, IPreviewHandler_Vtbl};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, ES_AUTOVSCROLL, ES_MULTILINE, ES_READONLY, HMENU, MoveWindow, MSG,
    SetWindowTextW, WS_BORDER, WS_CHILD, WS_VISIBLE, WS_VSCROLL, WINDOW_EX_STYLE, WINDOW_STYLE,
};

pub(crate) const IID_IUNKNOWN: GUID = GUID::from_u128(0x00000000_0000_0000_c000_000000000046);
pub(crate) const IID_IPREVIEW_HANDLER: GUID = GUID::from_u128(0x8895b1c6_b41f_4c1c_a562_0d564250836f);
pub(crate) const IID_IOBJECT_WITH_SITE: GUID = GUID::from_u128(0xfc4801a3_2ba9_11cf_a229_00aa003d7352);
pub(crate) const IID_IOLE_WINDOW: GUID = GUID::from_u128(0x00000114_0000_0000_c000_000000000046);
pub(crate) const IID_IINITIALIZE_WITH_STREAM: GUID =
    GUID::from_u128(0xb824b49d_22ac_4161_ac8a_9916e8fa3f7f);

const E_POINTER: HRESULT = HRESULT(0x80004003_u32 as _);

static ACTIVE_OBJECTS: AtomicI32 = AtomicI32::new(0);
pub(crate) fn active_objects() -> i32 {
    ACTIVE_OBJECTS.load(Ordering::SeqCst)
}

struct HandlerState {
    parent_hwnd: HWND,
    child_hwnd: HWND,
    stream: Option<IStream>,
    site: Option<IUnknown>,
}

/// 预览处理器 COM 对象：5 个接口共用一个对象（经典多 vtable 布局）。
#[repr(C)]
pub struct PreviewHandlerObject {
    vtbl_preview: *const IPreviewHandler_Vtbl,
    vtbl_site: *const IObjectWithSite_Vtbl,
    vtbl_window: *const IOleWindow_Vtbl,
    vtbl_init: *const IInitializeWithStream_Vtbl,
    pub(crate) refcount: AtomicI32,
    state: Mutex<HandlerState>,
}

#[repr(C)]
pub struct IObjectWithSite_Vtbl {
    pub base__: IUnknown_Vtbl,
    pub SetSite: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    pub GetSite: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
}

#[repr(C)]
pub struct IInitializeWithStream_Vtbl {
    pub base__: IUnknown_Vtbl,
    pub Initialize: unsafe extern "system" fn(*mut c_void, *mut c_void, u32) -> HRESULT,
}

impl PreviewHandlerObject {
    pub fn new() -> *mut PreviewHandlerObject {
        ACTIVE_OBJECTS.fetch_add(1, Ordering::SeqCst);
        let obj = Box::new(PreviewHandlerObject {
            vtbl_preview: &PREVIEW_VTABLE,
            vtbl_site: &SITE_VTABLE,
            vtbl_window: &WINDOW_VTABLE,
            vtbl_init: &INIT_VTABLE,
            refcount: AtomicI32::new(1),
            state: Mutex::new(HandlerState {
                parent_hwnd: HWND(std::ptr::null_mut()),
                child_hwnd: HWND(std::ptr::null_mut()),
                stream: None,
                site: None,
            }),
        });
        Box::into_raw(obj)
    }

    unsafe fn from_vtbl_slot(this: *mut c_void, offset: usize) -> *mut PreviewHandlerObject {
        (this as *mut u8).sub(offset) as *mut PreviewHandlerObject
    }

    unsafe fn lock(&self) -> MutexGuard<'_, HandlerState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub unsafe fn query_interface(&self, riid: &GUID, ppv: *mut *mut c_void) -> HRESULT {
        if ppv.is_null() {
            return E_POINTER;
        }
        let base = (self as *const PreviewHandlerObject) as *const u8;
        if riid == &IID_IUNKNOWN || riid == &IID_IPREVIEW_HANDLER {
            *ppv = base as *mut c_void;
        } else if riid == &IID_IOBJECT_WITH_SITE {
            *ppv = base.add(8) as *mut c_void;
        } else if riid == &IID_IOLE_WINDOW {
            *ppv = base.add(16) as *mut c_void;
        } else if riid == &IID_IINITIALIZE_WITH_STREAM {
            *ppv = base.add(24) as *mut c_void;
        } else {
            return E_NOINTERFACE;
        }
        self.refcount.fetch_add(1, Ordering::SeqCst);
        S_OK
    }
}

impl Drop for PreviewHandlerObject {
    fn drop(&mut self) {
        ACTIVE_OBJECTS.fetch_sub(1, Ordering::SeqCst);
    }
}

// ==================== IPreviewHandler ====================

unsafe extern "system" fn ph_qi(this: *mut c_void, riid: *const GUID, ppv: *mut *mut c_void) -> HRESULT {
    if riid.is_null() {
        return E_POINTER;
    }
    let obj = PreviewHandlerObject::from_vtbl_slot(this, 0);
    (*obj).query_interface(&*riid, ppv)
}

unsafe extern "system" fn ph_addref(this: *mut c_void) -> u32 {
    let obj = PreviewHandlerObject::from_vtbl_slot(this, 0);
    (*obj).refcount.fetch_add(1, Ordering::SeqCst) as u32 + 1
}

unsafe extern "system" fn ph_release(this: *mut c_void) -> u32 {
    let obj = PreviewHandlerObject::from_vtbl_slot(this, 0);
    let remaining = (*obj).refcount.fetch_sub(1, Ordering::SeqCst) as u32 - 1;
    if remaining == 0 {
        drop(Box::from_raw(obj));
    }
    remaining
}

unsafe extern "system" fn ph_set_window(this: *mut c_void, hwnd: HWND, rect: *const RECT) -> HRESULT {
    let obj = PreviewHandlerObject::from_vtbl_slot(this, 0);
    let mut state = (*obj).lock();
    state.parent_hwnd = hwnd;
    if let Some(child) = ensure_child(&mut state) {
        let r = if rect.is_null() {
            RECT { left: 0, top: 0, right: 800, bottom: 600 }
        } else {
            *rect
        };
        let _ = MoveWindow(child, r.left, r.top, r.right - r.left, r.bottom - r.top, true);
    }
    S_OK
}

unsafe extern "system" fn ph_set_rect(this: *mut c_void, rect: *const RECT) -> HRESULT {
    let obj = PreviewHandlerObject::from_vtbl_slot(this, 0);
    let state = (*obj).lock();
    if !state.child_hwnd.0.is_null() && !rect.is_null() {
        let r = *rect;
        let _ = MoveWindow(state.child_hwnd, r.left, r.top, r.right - r.left, r.bottom - r.top, true);
    }
    S_OK
}

unsafe extern "system" fn ph_do_preview(this: *mut c_void) -> HRESULT {
    let obj = PreviewHandlerObject::from_vtbl_slot(this, 0);
    {
        let state = (*obj).lock();
        if state.parent_hwnd.0.is_null() {
            return S_OK;
        }
    }
    let mut state = (*obj).lock();
    if let Some(child) = ensure_child(&mut state) {
        let text = render_preview(&state);
        set_edit_text(child, &text);
    }
    S_OK
}

unsafe extern "system" fn ph_unload(this: *mut c_void) -> HRESULT {
    let obj = PreviewHandlerObject::from_vtbl_slot(this, 0);
    let mut state = (*obj).lock();
    state.stream = None;
    state.site = None;
    if !state.child_hwnd.0.is_null() {
        let _ = SetWindowTextW(state.child_hwnd, PCWSTR(w!("").as_ptr()));
    }
    S_OK
}

unsafe extern "system" fn ph_set_focus(_this: *mut c_void) -> HRESULT {
    S_OK
}

unsafe extern "system" fn ph_query_focus(this: *mut c_void, phwnd: *mut HWND) -> HRESULT {
    let obj = PreviewHandlerObject::from_vtbl_slot(this, 0);
    let state = (*obj).lock();
    if phwnd.is_null() {
        return E_POINTER;
    }
    *phwnd = state.child_hwnd;
    S_OK
}

unsafe extern "system" fn ph_translate_accelerator(_this: *mut c_void, _pmsg: *const MSG) -> HRESULT {
    windows::Win32::Foundation::S_FALSE
}

// ==================== IObjectWithSite ====================

unsafe extern "system" fn site_qi(this: *mut c_void, riid: *const GUID, ppv: *mut *mut c_void) -> HRESULT {
    if riid.is_null() {
        return E_POINTER;
    }
    let obj = PreviewHandlerObject::from_vtbl_slot(this, 8);
    (*obj).query_interface(&*riid, ppv)
}

unsafe extern "system" fn site_addref(this: *mut c_void) -> u32 {
    let obj = PreviewHandlerObject::from_vtbl_slot(this, 8);
    (*obj).refcount.fetch_add(1, Ordering::SeqCst) as u32 + 1
}

unsafe extern "system" fn site_release(this: *mut c_void) -> u32 {
    let obj = PreviewHandlerObject::from_vtbl_slot(this, 8);
    let remaining = (*obj).refcount.fetch_sub(1, Ordering::SeqCst) as u32 - 1;
    if remaining == 0 {
        drop(Box::from_raw(obj));
    }
    remaining
}

unsafe extern "system" fn site_set_site(this: *mut c_void, punk: *mut c_void) -> HRESULT {
    let obj = PreviewHandlerObject::from_vtbl_slot(this, 8);
    let mut state = (*obj).lock();
    state.site = if punk.is_null() {
        None
    } else {
        let borrowed = IUnknown::from_raw(punk as *mut _);
        Some(borrowed.clone())
    };
    S_OK
}

unsafe extern "system" fn site_get_site(this: *mut c_void, riid: *const GUID, ppv: *mut *mut c_void) -> HRESULT {
    let obj = PreviewHandlerObject::from_vtbl_slot(this, 8);
    let state = (*obj).lock();
    if riid.is_null() || ppv.is_null() {
        return E_POINTER;
    }
    match &state.site {
        Some(site) => site.query(&*riid, ppv),
        None => E_NOINTERFACE,
    }
}

// ==================== IOleWindow ====================

unsafe extern "system" fn win_qi(this: *mut c_void, riid: *const GUID, ppv: *mut *mut c_void) -> HRESULT {
    if riid.is_null() {
        return E_POINTER;
    }
    let obj = PreviewHandlerObject::from_vtbl_slot(this, 16);
    (*obj).query_interface(&*riid, ppv)
}

unsafe extern "system" fn win_addref(this: *mut c_void) -> u32 {
    let obj = PreviewHandlerObject::from_vtbl_slot(this, 16);
    (*obj).refcount.fetch_add(1, Ordering::SeqCst) as u32 + 1
}

unsafe extern "system" fn win_release(this: *mut c_void) -> u32 {
    let obj = PreviewHandlerObject::from_vtbl_slot(this, 16);
    let remaining = (*obj).refcount.fetch_sub(1, Ordering::SeqCst) as u32 - 1;
    if remaining == 0 {
        drop(Box::from_raw(obj));
    }
    remaining
}

unsafe extern "system" fn win_get_window(this: *mut c_void, phwnd: *mut HWND) -> HRESULT {
    let obj = PreviewHandlerObject::from_vtbl_slot(this, 16);
    let state = (*obj).lock();
    if phwnd.is_null() {
        return E_POINTER;
    }
    *phwnd = state.parent_hwnd;
    S_OK
}

unsafe extern "system" fn win_context_sensitive_help(_this: *mut c_void, _fenter: BOOL) -> HRESULT {
    S_OK
}

// ==================== IInitializeWithStream ====================

unsafe extern "system" fn init_qi(this: *mut c_void, riid: *const GUID, ppv: *mut *mut c_void) -> HRESULT {
    if riid.is_null() {
        return E_POINTER;
    }
    let obj = PreviewHandlerObject::from_vtbl_slot(this, 24);
    (*obj).query_interface(&*riid, ppv)
}

unsafe extern "system" fn init_addref(this: *mut c_void) -> u32 {
    let obj = PreviewHandlerObject::from_vtbl_slot(this, 24);
    (*obj).refcount.fetch_add(1, Ordering::SeqCst) as u32 + 1
}

unsafe extern "system" fn init_release(this: *mut c_void) -> u32 {
    let obj = PreviewHandlerObject::from_vtbl_slot(this, 24);
    let remaining = (*obj).refcount.fetch_sub(1, Ordering::SeqCst) as u32 - 1;
    if remaining == 0 {
        drop(Box::from_raw(obj));
    }
    remaining
}

unsafe extern "system" fn init_initialize(this: *mut c_void, pstream: *mut c_void, _grfmode: u32) -> HRESULT {
    let obj = PreviewHandlerObject::from_vtbl_slot(this, 24);
    let mut state = (*obj).lock();
    state.stream = if pstream.is_null() {
        None
    } else {
        // from_raw 不增加引用计数（0.58 的 clone 也只是复制指针）；
        // 手动 AddRef 以匹配包装 drop 时的 Release，保证 handler 长期持有的引用有效
        let vtbl = *(pstream as *const *const IUnknown_Vtbl);
        ((*vtbl).AddRef)(pstream);
        Some(IStream::from_raw(pstream as *mut _))
    };
    S_OK
}

// ==================== Vtables ====================

static PREVIEW_VTABLE: IPreviewHandler_Vtbl = IPreviewHandler_Vtbl {
    base__: IUnknown_Vtbl {
        QueryInterface: ph_qi,
        AddRef: ph_addref,
        Release: ph_release,
    },
    SetWindow: ph_set_window,
    SetRect: ph_set_rect,
    DoPreview: ph_do_preview,
    Unload: ph_unload,
    SetFocus: ph_set_focus,
    QueryFocus: ph_query_focus,
    TranslateAccelerator: ph_translate_accelerator,
};

static SITE_VTABLE: IObjectWithSite_Vtbl = IObjectWithSite_Vtbl {
    base__: IUnknown_Vtbl {
        QueryInterface: site_qi,
        AddRef: site_addref,
        Release: site_release,
    },
    SetSite: site_set_site,
    GetSite: site_get_site,
};

static WINDOW_VTABLE: IOleWindow_Vtbl = IOleWindow_Vtbl {
    base__: IUnknown_Vtbl {
        QueryInterface: win_qi,
        AddRef: win_addref,
        Release: win_release,
    },
    GetWindow: win_get_window,
    ContextSensitiveHelp: win_context_sensitive_help,
};

static INIT_VTABLE: IInitializeWithStream_Vtbl = IInitializeWithStream_Vtbl {
    base__: IUnknown_Vtbl {
        QueryInterface: init_qi,
        AddRef: init_addref,
        Release: init_release,
    },
    Initialize: init_initialize,
};

// ==================== 渲染逻辑 ====================

fn ensure_child(state: &mut HandlerState) -> Option<HWND> {
    if !state.child_hwnd.0.is_null() {
        return Some(state.child_hwnd);
    }
    let style = WS_CHILD
        | WS_VISIBLE
        | WS_BORDER
        | WS_VSCROLL
        | WINDOW_STYLE(ES_MULTILINE as u32)
        | WINDOW_STYLE(ES_READONLY as u32)
        | WINDOW_STYLE(ES_AUTOVSCROLL as u32);
    let child = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            w!("EDIT"),
            w!(""),
            style,
            0,
            0,
            0,
            0,
            state.parent_hwnd,
            HMENU(1 as *mut c_void),
            windows::Win32::Foundation::HINSTANCE(0 as *mut c_void),
            None,
        )
    };
    match child {
        Ok(hwnd) => {
            state.child_hwnd = hwnd;
            Some(hwnd)
        }
        Err(_) => None,
    }
}

fn set_edit_text(hwnd: HWND, text: &str) {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let _ = unsafe { SetWindowTextW(hwnd, PCWSTR(wide.as_ptr())) };
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (i, ch) in text.chars().enumerate() {
        if i >= max_chars {
            out.push_str("\r\n\r\n[内容过长，预览已截断]");
            return out;
        }
        out.push(ch);
    }
    out
}

fn render_preview(state: &HandlerState) -> String {
    let Some(stream) = &state.stream else {
        return "无法预览此文件".to_string();
    };
    let mut buf = vec![0u8; 2 * 1024 * 1024];
    let mut read = 0u32;
    let hr = unsafe { stream.Read(buf.as_mut_ptr() as *mut c_void, buf.len() as u32, Some(&mut read)) };
    if hr != S_OK {
        return "无法预览此文件".to_string();
    }
    buf.truncate(read as usize);
    let text = crate::decode_preview(&buf);
    truncate_chars(&text, 30_000)
}
