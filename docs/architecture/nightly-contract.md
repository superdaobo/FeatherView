# 览匣 FeatherView 夜间架构合同（Nightly Contract）

> 版本：1.0（2026-08-02）
> 所有子 Agent 必须遵守本合同的协议定义。禁止自行发明互不兼容的协议。
> 需要修改合同 → 通过 dependency-requests 向主 Agent 申请。

## 1. DocumentSource 与文件唯一标识

沿用 v0.1.1 现有类型（`src/types/index.ts`），不破坏：

```ts
interface DocumentSource {
  id: string
  name: string
  extension: string
  path?: string
  uri?: string
  size?: number
  mimeType?: string
}
```

唯一标识继续使用 `platformService.getDocumentIdentity(source)`（Windows path 小写归一；URI 原样）。

## 2. 文本范围读取协议（大文件）

Rust 命令（`commands/file.rs` 新增）：

```rust
// 打开范围读取会话（返回 sessionId）
fn open_read_session(path: String) -> Result<String, AppError>
// 读取指定范围（分块）
fn read_range(sessionId: String, offset: u64, length: u32) -> Result<ReadRangeResult, AppError>
// 取消并关闭会话
fn close_read_session(sessionId: String) -> Result<(), AppError>
// 获取文件元信息（含 mtime，用于变化检测）
fn file_metadata(path: String) -> Result<FileMeta, AppError>  // 已有
```

```ts
interface ReadRangeResult {
  bytesRead: number
  nextOffset: number
  eof: boolean
  text: string        // 已按检测编码转码 UTF-8
  encoding: string
  sessionModifiedAt?: number  // 会话打开时的 mtime
}
```

规则：
- 会话打开时记录文件 mtime + 大小；`read_range` 每次校验当前 mtime，变化则返回 `FILE_CHANGED` 错误并关闭会话
- 编码：会话级一次性检测（前 8KB），UTF-8/UTF-16 按字符边界对齐（切块位置回退到上一个完整字符）；GBK/GB18030 采用"读取范围 + 前后各 4 字节冗余并丢弃不完整尾字节"策略，文档说明见 `docs/architecture/large-file-reading.md`
- 单次 `length` 上限 1MB；前端按需请求
- 取消：前端调 `close_read_session`；Rust 侧读取中不阻塞 UI（命令本身快）

## 3. 大文件读取返回结构（兼容现有 read_file）

`read_file` 保持不动（≤10MB 文本完整读取）。新增：

```ts
interface LargeFileInfo {
  size: number
  mtime: number
  encoding: string
  isText: boolean
  suggestedChunkBytes: number  // 默认 256KB
}
```

`open_read_session` 返回 `LargeFileInfo` + sessionId。文本渲染器据此决定是否走虚拟滚动路径（阈值：>1MB 或 >200k 行）。

## 4. 取消读取协议

```ts
// 前端可取消的读取统一走会话；取消 = close_read_session
interface ReadCancelToken { cancelled: boolean }
```

前端虚拟滚动渲染器持有 sessionId；组件卸载 / 标签关闭 / 文件切换时**必须** `close_read_session`。禁止泄漏会话（Rust 侧 session map 上限 32，超限淘汰最旧）。

## 5. Renderer 接口（不破坏现有）

```ts
interface RendererDefinition {
  id: string
  name: string
  extensions: string[]
  canHandle(document: DocumentMeta): boolean
  component: Component  // 动态 import 懒加载
}
```

新增渲染器（PDF/CSV/Archive）必须注册到 `src/renderers/index.ts`（主 Agent 合并）。PDF 渲染器 props 扩展约定：`{ document, content, bytesBase64?, fileUrl? }`，其中 `fileUrl` 仅 PDF 使用（经 Rust 转临时 asset 或本地 file:// 读取，由 Agent E 在依赖申请中说明方案）。

## 6. 标签页状态结构

```ts
interface TabState {
  tabId: string
  source: DocumentSource
  meta?: DocumentMeta
  rendererId?: string
  position?: ReadingPositionRecord  // 复用 v0.1.1 类型
  pinned?: boolean
}
```

store：`src/stores/tabs.ts`（Agent D 创建），`documentStore` 保持为"当前激活标签的视图状态"；标签切换 = documentStore.open 同源不重读（通过 `source.id` 相同且已有 result 时直接复用）。

## 7. 文件操作错误结构

复用 `AppError { title, message, code }`。新增 code：

```
FILE_ALREADY_EXISTS / FILE_CONFLICT（冲突，前端询问覆盖/跳过/重命名）
FILE_IN_USE（占用）
FILE_NOT_EMPTY / DIR_NOT_EMPTY
OPERATION_CANCELLED
ARCHIVE_UNSAFE_PATH（Zip Slip 拦截）
ARCHIVE_TOO_LARGE（解压炸弹/条目上限）
```

删除语义：默认回收站（`trash` crate 或 `recycle-bin`）；永久删除必须前端二次确认后走专用命令 `delete_permanent`。

## 8. 压缩包条目结构

```ts
interface ArchiveEntry {
  path: string        // 条目内路径（正斜杠）
  isDir: boolean
  size: number
  compressedSize?: number
  modifiedAt?: number
}
interface ArchiveInfo {
  format: 'zip' | 'tar' | 'gz' | 'tar.gz'
  entries: ArchiveEntry[]
  totalUncompressedSize: number
  truncated: boolean   // 超过条目上限被截断
}
```

限制（默认）：条目 ≤ 20000；单文件 ≤ 2GB；预计解压总大小 ≤ 10GB（超限拒绝）；解压目标必须解析后位于目标目录内（防 Zip Slip/绝对路径/符号链接逃逸）。冲突策略由前端询问（覆盖/跳过/重命名）。

## 9. Android URI 处理策略

- 文件选择：`tauri-plugin-dialog` 在 Android 返回 `content://` URI → `DocumentSource.uri`
- 读取：Rust 命令 `read_uri(uri, offset?, length?)`（Android 侧经 `ContentResolver` openInputStream；转临时文件或流式读取）
- 近期文件/阅读位置：`getDocumentIdentity` 以 URI 为标识
- 平台能力降级：无 path 时禁用需要 path 的操作（文件管理/收藏路径类），UI 层按 `source.path === undefined` 隐藏

## 10. iOS URL 与权限策略

- 文件 App 导入 / 分享打开：`file://` URL → `DocumentSource.uri`
- 读取：Security-Scoped Resource（`startAccessingSecurityScopedResource`）→ 临时拷贝到 app 容器再读，或直接读取；由 Agent G 在 iOS 侧实现并文档化
- 能力降级同 Android

## 11. 外部打开事件结构

沿用 v0.1.1：Rust `emit("external-file-open", args: Vec<String>)`；前端 `useExternalFileOpen` 统一处理。移动端新增 `emit("external-uri-open", uri: String)`（Android/iOS 分享打开），前端监听同一 handler 入口。

## 12. Agent 调试接口（Agent Test Mode）

- 启动参数：`--agent-mode --fixture-root=<path> --agent-port=<port>`（port=0 自动分配）
- 仅开发构建启用（`cfg!(debug_assertions)` 或显式 feature `agent-mode`；Release 默认关闭）
- 极简 HTTP 服务（Rust std TcpListener，仅绑定 127.0.0.1）：
  - `GET /health` → `{"ok":true}`
  - `GET /state` → 当前 route/source/status
  - `GET /logs` → 最近日志（脱敏：不输出文件内容）
  - `POST /open-file {path}`、`POST /open-fixture {name}`（仅 fixture-root 内）
  - `POST /navigate {route}`、`POST /reset-settings`、`POST /clear-storage`、`POST /simulate-error {code}`、`POST /shutdown`
- 认证：随机 token（启动时生成，写入 session 文件），请求头 `X-Agent-Token`
- 会话文件：`.reasonix/autoresearch/<task>/nightly-20260802/runtime/agent-session.json`
- 禁止：shell 执行、任意文件读写（仅 fixture-root）、远程绑定、真实文件内容入日志

## 13. Windows Preview Handler 共享边界

- `crates/featherview-core`：文本读取 + 编码检测 + 二进制判定（从 `src-tauri/src/document` 提取，无 Tauri 依赖，纯库）
- `src-tauri` 与 `crates/windows-preview-handler` 均依赖 featherview-core
- Preview Handler 不引入 WebView2 渲染 Markdown（P0 用原生文本控件 + 简单格式化）；WebView2 富预览为 P1 方向
- COM 实现用 Rust `windows` crate（`windows-core` + `windows` 的 `Win32::UI::Shell::PreviewHandler` 接口）

## 14. 平台功能检测接口

```ts
// platformService 新增
interface PlatformCapabilities {
  fileManagement: boolean   // 复制/移动/重命名/删除（false：Android/iOS content 模式）
  folderBrowsing: boolean   // 文件夹浏览（桌面 true）
  nativePath: boolean       // source.path 可用
  previewHandler: boolean   // Windows 桌面 true（仅信息用途）
}
function getPlatformCapabilities(): PlatformCapabilities
```

UI 按 capabilities 降级（隐藏不可用操作），不允许调用后报错白屏。

## 15. Release 构建中禁用调试能力

- Agent Mode 代码用 `#[cfg(any(debug_assertions, feature = "agent-mode"))]` 包裹
- nightly/alpha 构建可显式开 feature；release.yml 的稳定构建不开
- 前端不包含任何 agent 面板（仅命令行/HTTP 接口）

## 16. 版本与命名

- 显示名：`览匣 FeatherView`；应用标识 `com.featherview.app` 不变；crate 名 `featherview` 不变
- Nightly 版本：`0.4.0-alpha.1+<sha>`（以实际完成度定 0.2/0.3/0.4 alpha，不创建稳定 Tag）
- Preview Handler DLL 名：`LanxiaPreviewHandler.dll`；CLSID 固定 UUID（Agent B 生成并在文档登记）
