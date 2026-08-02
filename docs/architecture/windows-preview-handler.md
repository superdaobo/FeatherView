# Windows Preview Handler（资源管理器预览窗格）架构

> 状态：P0 设计定稿（2026-08-02，Agent B）。**本机未做 Explorer 实机验证**（环境限制），
> 以 `crates/preview-host` 测试宿主验证为准；§6 提供实机验证步骤，须在真实 Windows 上执行。
> 合同：`docs/architecture/nightly-contract.md` §13/§16。

## 1. 目标与范围

让 Windows 资源管理器右侧预览窗格直接显示 `.md / .markdown / .mdown / .txt / .log / .json /
.yaml / .yml / .toml / .csv` 文本内容（无需打开应用）。

- **P0（本轮）**：原生文本控件渲染（无 WebView2）、编码检测、2MB 读取上限 + 30000 字符显示截断、
  二进制/损坏文件显示「无法预览此文件」、不崩溃。
- **P1 方向**：WebView2 富预览（Markdown 渲染）、代码高亮（合同 §16 提及 P2 代码高亮 Preview Handler 的雏形）。

## 2. 组件与代码位置

| 组件 | crate | 形态 | 职责 |
| --- | --- | --- | --- |
| 核心库 | `crates/featherview-core` | rlib | 编码检测（BOM/UTF-16 启发/UTF-8/chardetng）、二进制判定、文件读取+截断 |
| 预览处理器 | `crates/windows-preview-handler` | cdylib（`lanxia_preview_handler.dll`，集成重命名 `LanxiaPreviewHandler.dll`） | COM 进程内服务器 |
| 测试宿主 | `crates/preview-host` | exe | 免注册验证 COM 链路（LoadLibrary + DllGetClassObject）或 `--via-com`（CoCreateInstance） |

依赖：`windows 0.62.2` + `windows-core 0.62.2`（crates.io 2026-10 最新稳定，记录于 Agent B 报告）；
COM 实现用 `#[implement]` 宏（0.62 中为独立 windows-implement crate，实现 trait 目标为 `X_Impl`）。

## 3. COM 接口与生命周期

### 3.1 接口

| 接口 | 位置（0.62） | 方法 | P0 行为 |
| --- | --- | --- | --- |
| `IInitializeWithStream` | `Win32::System::Com` | `Initialize(stream, grfmode)` | 保存 IStream 引用（宿主注入数据） |
| `IPreviewHandler` | `Win32::UI::Shell` | `SetWindow(hwnd, prc)` / `SetRect(prc)` / `DoPreview()` / `Unload()` / `SetFocus()` / `QueryFocus()` / `TranslateAccelerator(msg)` | 见 §3.3 |
| `IObjectWithSite` | `Win32::System::Com` | `SetSite` / `GetSite` | 保存/返回宿主 site |
| `IOleWindow` | `Win32::System::Ole` | `GetWindow()` / `ContextSensitiveHelp()` | 返回宿主窗口；帮助返回 E_NOTIMPL |
| `IClassFactory` | `Win32::System::Com` | `CreateInstance` / `LockServer` | 类工厂；拒绝聚合 |

> 注：0.62 中 `IOleWindow::GetWindow` 为 retval 提升（`-> Result<HWND>`），`IPreviewHandler::QueryFocus`
> 同理（`-> Result<HWND>`）；实现 trait（`*_Impl`）签名规则：结构体参数 → `&T`、指针参数 → 原值、
> 接口参数 → `Ref<T>`、out 参数 → `&mut T`。

### 3.2 对象生命周期
