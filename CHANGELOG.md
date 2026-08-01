# Changelog

FeatherView 轻阅 的版本变更记录。格式遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)。

## [0.1.1] - 2026-08-01

### Added

- Windows 文件关联支持（`.md` / `.markdown` / `.mdown` / `.txt` / `.log` / `.json` / `.yaml` / `.yml` / `.toml`）
- 单实例外部文件打开（应用已运行时双击关联文件，由现有实例打开并聚焦窗口）
- 阅读位置记忆与恢复（Markdown / TXT / LOG / 代码 / JSON，滚动防抖保存，最多 150 条）
- 设置项「记住阅读位置」（默认开启）与「清除阅读位置记录」
- 冷启动参数解析：中文路径、带空格路径、带引号路径、多参数只打开第一个

### Improved

- 拖拽提示覆盖层文案与文件夹提示
- 窗口标题随文档同步（`文件名 — FeatherView`）
- 打开文件期间的加载状态与重复打开防护
- 恢复阅读位置时显示一次克制提示

### Fixed

- 重复文件打开事件与最近文件重复记录
- 启动参数边界情况（Flag、不存在路径、URL、空参数）
- 渲染器容器晚于组件挂载时滚动监听丢失的问题

## [0.1.0] - 2026-07

首个 MVP 版本。

### Added

- Markdown / 纯文本 / 代码 / JSON / 图片阅读器（渲染器注册表 + 懒加载）
- 首页、最近文件（20 条）、文档搜索（Ctrl+F）、阅读设置、深浅主题
- 多编码支持（UTF-8 ±BOM / UTF-16 LE/BE ±BOM / GBK / GB18030）
- 统一错误处理与全局错误边界
- GitHub Actions CI 与 Release 工作流
