# Tauri iOS 工程配置（FeatherView）

> 作者：Agent G（夜间任务 2026-08-02）。本机为 Windows，**未实机验证**；
> 工程初始化与构建由 macOS GitHub Actions 完成（见 `.github/workflows/ios-job.yml`）。
> 关联文档：`docs/architecture/nightly-contract.md` 第 10/11/14 节、
> `docs/architecture/mobile-file-access.md`（iOS 章节）。

## 1. 前置条件

- macOS（Apple Silicon 或 Intel）+ Xcode 15+（`xcode-select --install`）
- Rust（`rustup`）+ 目标 `aarch64-apple-ios`：`rustup target add aarch64-apple-ios`
- pnpm 10 + Node 22；`@tauri-apps/cli`（package.json devDependencies 已有）
- iOS 图标：`pnpm tauri icon <app-icon.png> --ios-color '#fff'` 生成（init 会自动引用）
- Windows / Linux **无法**执行 `tauri ios init`（iOS 工程生成依赖 Xcode/cargo-mobile2）

## 2. tauri ios init 步骤与生成物
