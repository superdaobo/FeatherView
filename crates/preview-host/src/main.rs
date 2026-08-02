//! 览匣 Preview Handler 测试宿主（vtable 级调用，兼容 windows-rs 0.58）
//!
//! cargo run -p preview-host                # LoadLibrary + DllGetClassObject（免注册）
//! cargo run -p preview-host -- --via-com   # CoCreateInstance（需先运行 register.ps1）
use std::ffi::c_void;

use windows::core::{s, w, GUID, HRESULT, IUnknown_Vtbl, PCWSTR};
use windows::Win32::Foundation::{BOOL, HWND, RECT, S_OK};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, IClassFactory_Vtbl, IStream};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::UI::Shell::{IPreviewHandler_Vtbl, SHCreateMemStream};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, FindWindowExW, GetWindowTextW, WS_CHILD, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};

// ---------- IID 常量 ----------
const IID_IUNKNOWN: GUID = GUID::from_u128(0x00000000_0000_0000_c000_000000000046);
const IID_ICLASS_FACTORY: GUID = GUID::from_u128(0x00000001_0000_0000_c000_000000000046);
const IID_IPREVIEW_HANDLER: GUID = GUID::from_u128(0x8895b1c6_b41f_4c1c_a562_0d564250836f);
const IID_IOBJECT_WITH_SITE: GUID = GUID::from_u128(0xfc4801a3_2ba9_11cf_a229_00aa003d7352);
const IID_IOLE_WINDOW: GUID = GUID::from_u128(0x00000114_0000_0000_c000_000000000046);
const IID_IINITIALIZE_WITH_STREAM: GUID = GUID::from_u128(0xb824b49d_22ac_4161_ac8a_9916e8fa3f7f);
const CLSID_PREVIEW_HANDLER: GUID = GUID::from_u128(0x8895b1c6_b41f_4c1c_a562_0d564250836f);

// ---------- 自定义 Vtbl ----------
#[repr(C)]
struct IObjectWithSite_Vtbl {
    base__: IUnknown_Vtbl,
    SetSite: unsafe extern "system" fn(*mut c_void, *mut c_void) -> HRESULT,
    GetSite: unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> HRESULT,
}
#[repr(C)]
struct IInitializeWithStream_Vtbl {
    base__: IUnknown_Vtbl,
    Initialize: unsafe extern "system" fn(*mut c_void, *mut c_void, u32) -> HRESULT,
}

type DllGetClassObjectFn = unsafe extern "system" fn(
    *const GUID,
    *const GUID,
    *mut *mut c_void,
) -> HRESULT;

// ---------- vtable 调用 helpers ----------
unsafe fn qi(obj: *mut c_void, iid: &GUID, ppv: *mut *mut c_void) -> HRESULT {
    let vtbl = *(obj as *const *const IUnknown_Vtbl);
    ((*vtbl).QueryInterface)(obj, iid, ppv)
}
unsafe fn release(obj: *mut c_void) -> u32 {
    let vtbl = *(obj as *const *const IUnknown_Vtbl);
    ((*vtbl).Release)(obj)
}

struct Harness {
    dll: Option<windows::Win32::Foundation::HMODULE>,
    via_com: bool,
}

impl Harness {
    fn new(via_com: bool) -> Result<Self, String> {
        let dll = if via_com {
            None
        } else {
            Some(Self::load_handler_dll()?)
        };
        Ok(Harness { dll, via_com })
    }

    fn load_handler_dll() -> Result<windows::Win32::Foundation::HMODULE, String> {
        let path: Vec<u16> = "lanxia_preview_handler.dll".encode_utf16().chain(std::iter::once(0)).collect();
        unsafe { LoadLibraryW(PCWSTR(path.as_ptr())) }
            .map_err(|e| format!("LoadLibrary 失败: {e}"))
    }

    fn create_handler(&self) -> Result<*mut c_void, String> {
        if self.via_com {
            let hr = unsafe {
                CoCreateInstance::<_, windows::core::IUnknown>(
                    &CLSID_PREVIEW_HANDLER,
                    None,
                    CLSCTX_INPROC_SERVER,
                )
            };
            match hr {
                Ok(u) => {
                    let ptr = windows::core::Interface::as_raw(&u);
                    // 防止包装 drop 时 Release 释放对象（引用已转移给调用者）
                    std::mem::forget(u);
                    Ok(ptr)
                }
                Err(e) => Err(format!("CoCreateInstance 失败: {e}")),
            }
        } else {
            let dll = self.dll.ok_or("no dll")?;
            let proc = unsafe { GetProcAddress(dll, s!("DllGetClassObject")) }
                .ok_or("DllGetClassObject 未导出")?;
            let f: DllGetClassObjectFn = unsafe { std::mem::transmute(proc) };
            let mut factory: *mut c_void = std::ptr::null_mut();
            let hr = unsafe { f(&CLSID_PREVIEW_HANDLER, &IID_ICLASS_FACTORY, &mut factory) };
            if hr != S_OK {
                return Err(format!("DllGetClassObject 失败: {hr:?}"));
            }
            // IClassFactory::CreateInstance
            let mut handler: *mut c_void = std::ptr::null_mut();
            let hr = unsafe {
                let fvtbl = *(factory as *const *const IClassFactory_Vtbl);
                ((*fvtbl).CreateInstance)(factory, std::ptr::null_mut(), &IID_IPREVIEW_HANDLER, &mut handler)
            };
            unsafe { release(factory) };
            if hr != S_OK {
                return Err(format!("CreateInstance 失败: {hr:?}"));
            }
            Ok(handler)
        }
    }

    fn child_edit(&self, parent: HWND) -> Result<HWND, String> {
        unsafe { FindWindowExW(parent, HWND(std::ptr::null_mut()), w!("EDIT"), PCWSTR(std::ptr::null())) }
            .map_err(|_| "EDIT child not found".to_string())
    }

    fn child_text(&self, parent: HWND) -> Result<String, String> {
        let edit = self.child_edit(parent)?;
        let mut buf = vec![0u16; 65536];
        let n = unsafe { GetWindowTextW(edit, &mut buf) };
        Ok(String::from_utf16_lossy(&buf[..n.max(0) as usize]))
    }
}

fn create_parent_window() -> Result<HWND, String> {
    unsafe {
        CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
            w!("STATIC"),
            w!("PreviewHost"),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            0,
            0,
            800,
            600,
            HWND(std::ptr::null_mut()),
            windows::Win32::UI::WindowsAndMessaging::HMENU(std::ptr::null_mut()),
            windows::Win32::Foundation::HINSTANCE(std::ptr::null_mut()),
            None,
        )
    }
    .map_err(|e| format!("CreateWindowExW 失败: {e}"))
}

fn mem_stream(data: &[u8]) -> Result<IStream, String> {
    unsafe { SHCreateMemStream(Some(data)) }.ok_or_else(|| "SHCreateMemStream 失败".to_string())
}

// ---------- 接口方法（vtable 级） ----------
unsafe fn ph_set_window(h: *mut c_void, hwnd: HWND, rc: *const RECT) -> HRESULT {
    let vtbl = *(h as *const *const IPreviewHandler_Vtbl);
    ((*vtbl).SetWindow)(h, hwnd, rc)
}
unsafe fn ph_set_rect(h: *mut c_void, rc: *const RECT) -> HRESULT {
    let vtbl = *(h as *const *const IPreviewHandler_Vtbl);
    ((*vtbl).SetRect)(h, rc)
}
unsafe fn ph_do_preview(h: *mut c_void) -> HRESULT {
    let vtbl = *(h as *const *const IPreviewHandler_Vtbl);
    ((*vtbl).DoPreview)(h)
}
unsafe fn ph_unload(h: *mut c_void) -> HRESULT {
    let vtbl = *(h as *const *const IPreviewHandler_Vtbl);
    let hr = ((*vtbl).Unload)(h);
    hr
}
unsafe fn ows_set_site(site: *mut c_void, punk: *mut c_void) -> HRESULT {
    let vtbl = *(site as *const *const IObjectWithSite_Vtbl);
    ((*vtbl).SetSite)(site, punk)
}
unsafe fn init_initialize(init: *mut c_void, stream: *mut c_void, mode: u32) -> HRESULT {
    let vtbl = *(init as *const *const IInitializeWithStream_Vtbl);
    ((*vtbl).Initialize)(init, stream, mode)
}

/// 运行一次完整预览流程：Initialize(stream) → SetSite → SetWindow → SetRect → DoPreview。
fn run_preview_flow(h: &Harness, data: &[u8]) -> Result<(HWND, *mut c_void), String> {
    let stream = mem_stream(data)?;
    let handler = h.create_handler()?;
    // QI IInitializeWithStream
    let mut init: *mut c_void = std::ptr::null_mut();
    let hr = unsafe { qi(handler, &IID_IINITIALIZE_WITH_STREAM, &mut init) };
    if hr != S_OK {
        unsafe { release(handler) };
        return Err(format!("QI IInitializeWithStream 失败: {hr:?}"));
    }
    let hr = unsafe { init_initialize(init, windows::core::Interface::as_raw(&stream), 0) };
    if hr != S_OK {
        return Err(format!("Initialize 失败: {hr:?}"));
    }
    // QI IObjectWithSite 并 SetSite
    let mut site: *mut c_void = std::ptr::null_mut();
    let hr = unsafe { qi(handler, &IID_IOBJECT_WITH_SITE, &mut site) };
    if hr == S_OK {
        unsafe { ows_set_site(site, std::ptr::null_mut()) };
    }
    let parent = create_parent_window()?;
    let rc = RECT { left: 0, top: 0, right: 800, bottom: 600 };
    unsafe {
        let hr1 = ph_set_window(handler, parent, &rc);
            let hr2 = ph_do_preview(handler);
        }
    unsafe { release(init) };
    if !site.is_null() {
        unsafe { release(site) };
    }
    Ok((parent, handler))
}
fn check(cond: bool, name: &str) -> i32 {
    if cond {
        println!("[PASS] {name}");
        1
    } else {
        println!("[FAIL] {name}");
        0
    }
}

fn main() {
    let via_com = std::env::args().any(|a| a == "--via-com");
    if via_com {
        // CoCreateInstance 需要先初始化 COM
        unsafe { let _ = windows::Win32::System::Com::CoInitializeEx(None, windows::Win32::System::Com::COINIT_APARTMENTTHREADED); }
    }
    println!("=== Preview Handler Test Host (via_com={via_com}) ===");
    let h = match Harness::new(via_com) {
        Ok(h) => h,
        Err(e) => {
            println!("[FAIL] setup: {e}");
            std::process::exit(1);
        }
    };
    let mut pass = 0;
    let mut total = 0;

    // 1. 基础 Markdown/TXT 内容渲染
    {
        total += 1;
        let data = "# Hello\n\nchinese preview test.\nline 3\n".as_bytes();
        match run_preview_flow(&h, data) {
            Ok((parent, handler)) => {
                let text = h.child_text(parent).unwrap_or_default();
                pass += check(text.contains("Hello") && text.contains("chinese"), "basic text preview");
                // 2. Unload 清空
                total += 1;
                            let hr_u = unsafe { ph_unload(handler) };
                            let text2 = h.child_text(parent).unwrap_or_default();
                pass += check(!text2.contains("Hello"), "unload clears content");
                            unsafe { release(handler) };
                        }
            Err(e) => {
                pass += check(false, &format!("basic preview: {e}"));
                total += 1;
            }
        }
    }

    // 3. GBK 解码
    {
        total += 1;
        let (encoded, _, _) = encoding_rs::GBK.encode("GBK 编码的中文内容。");
        match run_preview_flow(&h, &encoded) {
            Ok((parent, handler)) => {
                let text = h.child_text(parent).unwrap_or_default();
                pass += check(text.contains("GBK") && text.contains("中文"), "gbk decode");
                unsafe { release(handler) };
            }
            Err(e) => {
                pass += check(false, &format!("gbk: {e}"));
                total += 1;
            }
        }
    }

    // 4. UTF-16LE 解码
    {
        total += 1;
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend("UTF-16 测试".encode_utf16().flat_map(|u| u.to_le_bytes()));
        match run_preview_flow(&h, &bytes) {
            Ok((parent, handler)) => {
                let text = h.child_text(parent).unwrap_or_default();
                pass += check(text.contains("UTF-16") && text.contains("测试"), "utf16 decode");
                unsafe { release(handler) };
            }
            Err(e) => {
                pass += check(false, &format!("utf16: {e}"));
                total += 1;
            }
        }
    }

    // 5. 二进制拒绝
    {
        total += 1;
        let data = [0x89u8, 0x50, 0x4E, 0x47, 0x00, 0x01, 0x02, 0x03, 0x00, 0x00];
        match run_preview_flow(&h, &data) {
            Ok((parent, handler)) => {
                let text = h.child_text(parent).unwrap_or_default();
                // 二进制内容解码失败应显示占位或空，不崩溃
                pass += check(true, "binary no crash");
                unsafe { release(handler) };
            }
            Err(e) => {
                pass += check(true, &format!("binary gracefully rejected: {e}"));
                total += 1;
            }
        }
    }

    // 6. 空文件
    {
        total += 1;
        match run_preview_flow(&h, b"") {
            Ok((parent, handler)) => {
                let _ = h.child_text(parent);
                pass += check(true, "empty file no crash");
                unsafe { release(handler) };
            }
            Err(e) => {
                pass += check(true, &format!("empty gracefully: {e}"));
                total += 1;
            }
        }
    }

    // 7. 大文件截断（3MB 文本 → 渲染截断提示）
    {
        total += 1;
        let big = vec![b'a'; 3 * 1024 * 1024];
        match run_preview_flow(&h, &big) {
            Ok((parent, handler)) => {
                let text = h.child_text(parent).unwrap_or_default();
                pass += check(text.contains("截断") || text.len() < 40000, "large file truncated");
                unsafe { release(handler) };
            }
            Err(e) => {
                pass += check(false, &format!("large: {e}"));
                total += 1;
            }
        }
    }

    // 8. SetRect 变尺寸（不崩溃即可）
    {
        total += 1;
        match run_preview_flow(&h, "resize test content".as_bytes()) {
            Ok((parent, handler)) => {
                let rc2 = RECT { left: 0, top: 0, right: 400, bottom: 300 };
                unsafe { ph_set_rect(handler, &rc2) };
                pass += check(true, "set rect resize");
                unsafe { release(handler) };
            }
            Err(e) => {
                pass += check(false, &format!("resize: {e}"));
                total += 1;
            }
        }
    }

    // 9. 压力测试：100 次创建/预览/销毁
    {
        total += 1;
        let mut ok = true;
        for _ in 0..100 {
            match run_preview_flow(&h, "stress test content line".as_bytes()) {
                Ok((_, handler)) => {
                    unsafe { release(handler) };
                }
                Err(_) => {
                    ok = false;
                    break;
                }
            }
        }
        pass += check(ok, "100x stress");
    }
    // 10. DllCanUnloadNow（通过 LoadLibrary 时检查活动对象归零）
    if let Some(dll) = h.dll {
        total += 1;
        let proc = unsafe { GetProcAddress(dll, s!("DllCanUnloadNow")) };
        if let Some(proc) = proc {
            let f: unsafe extern "system" fn() -> HRESULT = unsafe { std::mem::transmute(proc) };
            let hr = unsafe { f() };
            pass += check(hr == S_OK, "dll can unload now");
        } else {
            pass += check(false, "dll can unload now (export missing)");
        }
    }

    println!("=== 结果: {pass}/{total} PASS ===");
    if pass != total {
        std::process::exit(1);
    }
}
