# FeatherView / 轻阅

[![Build](https://github.com/superdaobo/FeatherView/actions/workflows/build.yml/badge.svg)](https://github.com/superdaobo/FeatherView/actions/workflows/build.yml)
[![Release](https://img.shields.io/github/v/release/superdaobo/FeatherView)](https://github.com/superdaobo/FeatherView/releases)
[![License](https://img.shields.io/github/license/superdaobo/FeatherView)](LICENSE)

轻量、本地优先、无账号、无广告的 Markdown 与常见文件阅读器。

基于 Tauri 2 + Vue 3 构建，支持 Windows（Android / iOS 架构兼容，后续版本发布）。

## 产品简介

FeatherView 是一款本地文件阅读器：启动快速、界面克制，打开文件即读即走。
文件保留在用户原来的位置，应用默认完全离线，不会上传、分析或收集任何文件内容。

## 当前 MVP 功能

- **首页**：打开文件（按钮 / 拖入窗口 / `Ctrl+O`）、最近打开列表（最多 20 条、失效提示、单条删除、清空）、支持格式说明、空状态
- **Markdown 阅读**：标题锚点、自动目录、任务列表、表格、引用、代码高亮与复制、相对路径图片、外部链接系统浏览器打开、缺失图片占位
- **纯文本 / 日志**：行号开关、自动换行、等宽字体、大文本截断保护
- **代码与配置文件**：20+ 语言按需注册的语法高亮、行号、自动换行、复制全文
- **JSON**：格式化 / 原始视图切换、格式错误提示
- **图片**：缩放（Ctrl+滚轮）、拖动、适应窗口、原始比例、基本信息
- **文档内搜索**：`Ctrl+F`、匹配计数、上一个 / 下一个、无结果状态
- **阅读设置**：深浅色主题（跟随系统）、字号（`Ctrl++` / `Ctrl+-` / `Ctrl+0`）、行高、内容宽度、字体族、自动换行、行号，即时生效并持久化
- **错误处理**：文件不存在 / 无权限 / 过大 / 编码识别失败等统一友好提示，全局错误边界不白屏

## 支持格式

| 类别 | 扩展名 |
| --- | --- |
| Markdown | `.md` `.markdown` `.mdown` |
| 纯文本 / 日志 | `.txt` `.log` |
| 代码与配置 | `.js` `.ts` `.jsx` `.tsx` `.vue` `.css` `.html` `.htm` `.py` `.rs` `.java` `.c` `.cpp` `.h` `.hpp` `.cs` `.go` `.sh` `.ps1` `.yaml` `.yml` `.toml` `.xml` `.ini` `.env` `.sql` `.bat` `.cmd` |
| 结构化数据 | `.json`（格式化查看） |
| 图片 | `.png` `.jpg` `.jpeg` `.webp` `.gif` `.svg` `.bmp` `.ico` `.avif` |

编码支持：UTF-8（含 BOM）、UTF-16 LE/BE（含 BOM 与无 BOM 启发式）、GBK / GB18030 等（chardetng 检测）。

## 截图

暂无截图（后续补充：`docs/screenshots/`）。

## 技术栈

- **桌面框架**：Tauri 2（WebView2）
- **前端**：Vue 3（Composition API + `<script setup lang="ts">`）、TypeScript、Vite、Pinia、Vue Router
- **Markdown**：markdown-it + markdown-it-anchor + markdown-it-task-lists + DOMPurify + highlight.js（按需注册）
- **图标**：lucide-vue-next 按需导入
- **Rust**：serde / serde_json / encoding_rs / chardetng / thiserror / base64
- **插件**：tauri-plugin-dialog（文件选择）、tauri-plugin-opener（外部链接）
- **样式**：原生 CSS + CSS Variables（无大型 UI 框架）

## 项目结构

```
├─ src/                      # 前端
│  ├─ components/            # （预留）
│  ├─ composables/           # useShortcuts / useToast / useDocumentSearch
│  ├─ pages/                 # HomePage / ReaderPage / SettingsPage
│  ├─ renderers/             # 渲染器注册表与实现（markdown/text/code/json/image/unsupported）
│  ├─ services/              # platformService / documentService / recentFilesService / settingsService
│  ├─ stores/                # Pinia：document / recentFiles / settings
│  ├─ styles/                # 主题变量与排版样式
│  ├─ test/                  # Vitest 环境初始化
│  ├─ types/                 # 共享类型定义
│  └─ utils/                 # 格式化等纯函数
├─ src-tauri/
│  ├─ capabilities/          # 最小权限配置
│  ├─ src/
│  │  ├─ commands/           # file.rs / system.rs（Tauri 命令）
│  │  ├─ document/           # 文件读取与编码检测
│  │  ├─ error.rs            # 统一用户可读错误
│  │  ├─ lib.rs              # 应用入口与命令注册
│  │  └─ main.rs
│  └─ tauri.conf.json
├─ tests/fixtures/           # 手动验收文件
└─ docs/                     # 实施计划与路线图
```

## 开发环境要求

- Windows 10/11（WebView2 已随系统更新）
- Node.js ≥ 20 与 pnpm ≥ 9
- Rust stable（MSVC toolchain）
- Visual Studio 2022 Build Tools（含 C++ 桌面开发）
- Android/iOS 构建额外需要：Android Studio + JDK、Xcode（仅 macOS）

## 安装与运行

```bash
# 安装依赖
pnpm install

# 前端开发模式（浏览器中仅界面，文件读取需 Tauri 环境）
pnpm dev

# Tauri 桌面开发模式（推荐）
pnpm tauri dev

# Windows 构建安装包（NSIS，产物在 src-tauri/target/release/bundle/nsis/）
pnpm tauri build
```

## 质量检查

```bash
pnpm lint
pnpm typecheck
pnpm test
pnpm build
cd src-tauri && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test
```

## Android / iOS

架构已兼容 Tauri 2 移动端：`DocumentSource` 同时支持 `path` 与 `uri`（Android `content://`、iOS `file://`），平台差异集中在 `platformService`。
本轮 MVP 不发布移动端安装包。

```bash
# 初始化移动端工程（需要 Android Studio / Xcode）
pnpm tauri android init
pnpm tauri ios init
```

## 快捷键

| 快捷键 | 功能 |
| --- | --- |
| `Ctrl+O` | 打开文件 |
| `Ctrl+F` | 当前文档搜索 |
| `Ctrl++` / `Ctrl+-` | 放大 / 缩小字号 |
| `Ctrl+0` | 恢复默认字号 |
| `Ctrl+Shift+T` | 切换主题 |
| `Esc` | 关闭搜索 / 抽屉 / 弹窗 |

## 已知限制

- 文本文件完整读取上限 10MB、二进制 50MB，超大文件暂不支持（分块读取在路线图中）
- 未实现大文件虚拟滚动（超过 200 万字符的文本自动截断预览）
- 未实现 PDF / Office / EPUB（渲染器接口已预留）
- 未实现文件夹浏览、多标签页、文件关联（右键打开）
- Android / iOS 安装包尚未构建

## 下一阶段计划

详见 [docs/roadmap.md](docs/roadmap.md)。

## 主要依赖及许可证

| 依赖 | 许可证 |
| --- | --- |
| Tauri 2 | MIT / Apache-2.0 |
| Vue 3 | MIT |
| Pinia | MIT |
| Vue Router | MIT |
| Vite | MIT |
| markdown-it | MIT |
| markdown-it-anchor | MIT |
| markdown-it-task-lists | MIT |
| DOMPurify | Apache-2.0 / MPL-2.0 |
| highlight.js | BSD-3-Clause |
| lucide-vue-next | ISC |
| serde / serde_json | MIT / Apache-2.0 |
| encoding_rs | MIT / Apache-2.0 |
| chardetng | MPL-2.0 / Apache-2.0 |
| thiserror | MIT / Apache-2.0 |
| base64 | MIT / Apache-2.0 |
| tauri-plugin-dialog / opener | MIT / Apache-2.0 |

本项目遵循 GPL-3.0 许可证，详见 [LICENSE](LICENSE)。

## 隐私说明

FeatherView 默认在本地处理文件。
应用不会上传、分析或收集用户打开的文件内容。

- 所有文件读取均在本地完成，无任何网络请求（外部链接需用户主动点击，且使用系统浏览器打开）
- 最近文件记录与阅读设置仅保存在本机 localStorage
- 日志中不包含文件内容

## 构建产物体积

- 前端首屏 bundle：约 129 KB（gzip 48 KB），渲染器全部懒加载
- Windows 可执行文件（GitHub Actions Release 构建）：约 3.1 MB
- Windows NSIS 安装包（GitHub Actions Release 构建）：约 1.3 MB
- 安装包可从 [Releases](https://github.com/superdaobo/FeatherView/releases) 下载
