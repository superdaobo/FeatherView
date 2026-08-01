# FeatherView MVP 实施计划

> 本文件为开发前制定的实施计划，记录各阶段目标与验证方式。

## 阶段 1：项目初始化与基础设施

- 初始化 Tauri 2 + Vue 3 + TypeScript + Vite + Pinia + Vue Router 项目（pnpm）
- 配置最小权限 capability（`core:default` + `dialog:allow-open` + `opener:default` + 自定义命令）
- 配置 NSIS 单安装包构建
- **验证**：`pnpm install`、`pnpm tauri dev` 冒烟启动

## 阶段 2：Rust 文件读取后端

- `error.rs`：AppError 统一错误（NotFound / PermissionDenied / TooLarge / Binary / UnknownEncoding / Io）
- `document/`：二进制检测（扩展名 + NUL 扫描）、10MB 文本 / 50MB 二进制读取上限、
  编码检测（UTF-8 BOM → UTF-16 启发式 → 严格 UTF-8 → chardetng → encoding_rs 转码）
- `commands/file.rs`：`read_file` / `file_exists` / `file_metadata` 命令
- `commands/system.rs`：`app_version` / `startup_args`（启动参数入口预留） / `platform_info`
- **验证**：`cargo test`（UTF-8/BOM/UTF-16/GBK/不存在/二进制/大文件）、`cargo clippy -D warnings`、`cargo fmt --check`

## 阶段 3：前端核心层

- `types/`：DocumentSource（path/uri 双轨，移动端预留）、RecentFile、ReaderSettings、ReadFileResult、RendererDefinition
- `services/platformService.ts`：统一文件入口（按钮选择 / 拖拽 / 启动参数），平台差异集中于此
- `services/documentService.ts`：invoke 封装与错误规范化
- `services/recentFilesService.ts` + store：20 条上限、排序去重、localStorage 持久化
- `services/settingsService.ts` + store：默认值（中文阅读友好）、持久化、主题应用
- `composables/useShortcuts.ts`：Ctrl+O/F/+/−/0/Shift+T/Esc（输入框不冲突）
- `composables/useToast.ts` + `composables/useDocumentSearch.ts`：通知与文档内搜索
- App.vue 全局错误边界 + 拖拽打开
- **验证**：`pnpm typecheck`、`pnpm test`

## 阶段 4：渲染器架构与实现

- `renderers/registry.ts`：`RendererDefinition { id/name/extensions/canHandle/component }` + 匹配分发
- markdown：markdown-it → DOMPurify（html 关闭、禁 script）→ 阅读视图；highlight.js 按需注册；代码复制；相对图片；外部链接系统浏览器
- text / code / json / image / unsupported 渲染器，全部动态 import 懒加载
- **验证**：`pnpm test`（渲染器匹配、Markdown 安全单测）

## 阶段 5：页面与交互

- HomePage：打开文件、最近文件（失效提示/单删/清空）、空状态、支持格式说明
- ReaderPage：三栏自适应布局、工具栏、状态栏、搜索、目录、抽屉
- SettingsPage：主题/字号/行高/宽度/字体/换行/行号，即时生效
- CSS Variables 深浅双主题 + 跟随系统
- **验证**：`pnpm test`（组件测试）、`pnpm build`

## 阶段 6：测试、构建与交付

- `tests/fixtures/`：19 个手动验收文件（中文/英文/图片/功能 Markdown、多编码 TXT、代码、JSON、图片、SVG、不支持、损坏 JSON）
- 质量门禁：`pnpm lint` / `typecheck` / `test` / `build`、`cargo fmt --check` / `clippy -D warnings` / `test`
- `pnpm tauri build`（NSIS），记录安装包体积
- README / roadmap 文档，按阶段 git 提交并推送 GitHub
