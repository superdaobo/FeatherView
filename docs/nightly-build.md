# 览匣 FeatherView 夜间构建说明（Nightly Build）

## 什么是 Nightly

Nightly 是面向开发迭代的自动化构建通道，产物**不标记为稳定版本**，仅用于：
- 验证多平台（Windows / Android / iOS）可构建性
- 提供可安装的调试产物供测试
- 在正式版本发布前暴露集成问题

Nightly 构建由 `.github/workflows/nightly.yml` 驱动（已在 main 注册，用户批准）。

## 触发方式

```bash
# 手动触发（指定分支）
gh workflow run "Nightly Build" --ref autoresearch/nightly-lanxia

# 或 GitHub 页面 Actions → Nightly Build → Run workflow
```

每日 02:30 UTC 自动运行。

## Job 与产物

| Job | 平台 | 产物 |
| --- | --- | --- |
| check | ubuntu | 全量门禁：lint/typecheck/test/build + fmt/clippy/test |
| build-windows | windows-latest | `FeatherView_x64.exe` + NSIS 安装包 + 体积报告 |
| build-android | ubuntu + Android SDK/NDK | Debug APK（aarch64） |
| build-ios | macos-14 | 未签名 `.app` + `Lanxia-unsigned.ipa`（Payload/览匣.app） |

产物通过 GitHub Actions Artifacts 下载（保留 14 天），**不自动创建 Release**。

## iOS 未签名说明

- unsigned IPA 中的 `.app` 未签名：**不能直接安装到普通 iPhone**，**不能提交 App Store**
- 仅用于重签名、分析或保存构建产物

## 体积目标（对比）

| 项 | v0.1.1 | Nightly 目标 |
| --- | --- | --- |
| 首屏 gzip | 48.57 KB | ≤ 65 KB |
| PDF.js | 不进首屏 | 独立 chunk |
| Windows 安装包 | 1.65 MB | 合理增加（含新格式与移动端配置） |

## 本地复现

```bash
pnpm install && pnpm lint && pnpm typecheck && pnpm test && pnpm build
cd src-tauri && cargo fmt --check && cargo clippy --all-targets --all-features -- -D warnings && cargo test
```
