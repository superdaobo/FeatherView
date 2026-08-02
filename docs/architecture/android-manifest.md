# AndroidManifest 配置需求（FeatherView Android）

> 目标文件：`src-tauri/gen/android/app/src/main/AndroidManifest.xml`
> 该文件由 `pnpm tauri android init` 生成（tauri 2.11.5 模板），`scripts/android-init.ps1`
> 会幂等注入 VIEW intent-filter（见下文 §3）。**注意它是生成文件**：重跑 init 前请备份注入内容；
> tauri 模板渲染不会覆盖已存在的文件，但删除 `gen/android` 重跑 init 会丢失手工修改。

## 1. 模板默认结构（tauri 2.11.5 源码核实）

`tauri android init` 生成的 manifest 已包含（无需手工添加）：
