# 移动端文件访问（Mobile File Access）

> 对应夜间架构合同 §9/§10/§11/§14。Android 章节为本轮（Agent F）交付；iOS 章节为预留骨架（Agent G）。
> 相关代码：`src-tauri/src/commands/uri.rs`（read_uri 等命令）、`scripts/android/FeatherViewPlugin.kt`
> （Kotlin 桥接）、`src/services/androidAdapter.ts`（前端适配）、`docs/architecture/android-manifest.md`。

## 总览

移动端没有可写路径的文件系统语义，文件访问统一走 **URI 双轨**（`DocumentSource { path?, uri? }`）：

| 平台 | 来源 | DocumentSource 字段 | 读取命令 |
|---|---|---|---|
| Windows 桌面 | 路径（选择/拖拽/关联） | `path` | `read_file` |
| Android | SAF content://（选择器 / 打开方式 / 分享） | `uri`（+`mimeType`） | `read_uri`（经临时文件缓存） |
| iOS | 文件 App file://（导入 / 分享打开） | `uri` | `read_uri`（预留，Agent G） |

`getDocumentIdentity(source)`：path 走 Windows 归一；URI 原样（`uri:<raw>`）作为阅读位置/最近文件标识（合同 §9）。

---

## Android

### 1. SAF 与 content://

- 文件选择：`tauri-plugin-dialog`（2.7.2）在 Android 上经 SAF `ACTION_OPEN_DOCUMENT`
  返回 `content://` URI（官方 mobile.rs 源码核实）→ `androidAdapter.uriToSource(uri, mimeType)`。
- content:// 的特点：URI 由 `authority` + 文档 id 组成，**不是文件路径**；
  必须通过 `ContentResolver.openInputStream` 读取，且依赖调用方授予的 URI 权限。
- 唯一标识：URI 原样（`getDocumentIdentity` 返回 `uri:<raw>`）。

### 2. 打开方式

- Manifest 注入 VIEW intent-filter（文本/Markdown/JSON/YAML/TOML 等 mimeType，
  见 `docs/architecture/android-manifest.md` §3）；系统"打开方式"→ `ACTION_VIEW` + content:// data。
- 事件流：`FeatherViewPlugin.onNewIntent`（热启动）→ 双通道（`trigger("newIntent")` 事件 +
  `window.__featherviewAndroidUri(uri)` 直投）→ 前端 `androidAdapter.listenUriOpen(handler)`
  （同时监听 Rust `external-uri-open` 事件，合同 §11）→ 与 `useExternalFileOpen` 相同的打开流程。
- 分享接收（ACTION_SEND，EXTRA_STREAM）为 P1（manifest §4 已给方案，桥接代码已兼容）。

### 3. 读取链路（临时文件拷贝策略，合同 §9）
