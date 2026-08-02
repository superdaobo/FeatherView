# =============================================================================
# FeatherView（览匣）Android 工程初始化脚本
# -----------------------------------------------------------------------------
# 用途：
#   1. 检查 Android 构建前置条件（JDK / Android SDK / NDK / Rust target）
#   2. 执行 `tauri android init` 生成 src-tauri/gen/android 工程
#   3. 注入 Kotlin 桥接（FeatherViewPlugin.kt）与 AndroidManifest VIEW intent-filter
#   4. 打印后续构建命令
#
# 运行环境：Windows PowerShell 5.1+ / PowerShell 7（本机为 Windows 开发机）。
# Linux/macOS 与 CI 等价命令见 docs/architecture/android-manifest.md 与
# .github/workflows/android-job.yml（CI 由 GitHub Actions 完成，无需本脚本）。
#
# 用法：
#   .\scripts\android-init.ps1                          # 完整初始化
#   .\scripts\android-init.ps1 -SkipManifestPatch       # 不自动改 AndroidManifest
#   .\scripts\android-init.ps1 -SkipTargetsInstall      # 跳过 rustup target 安装
#
# 注意：
#   - 本机无 Android SDK 时脚本会在前置检查处失败，请先安装 SDK/NDK 或
#     依赖 GitHub Actions（见 android-job.yml）。
#   - 脚本幂等：可重复执行；已注入的 intent-filter / 已存在的 Kotlin 文件不会被覆盖。
# =============================================================================

[CmdletBinding()]
param(
    # 跳过 AndroidManifest.xml 的 VIEW intent-filter 自动注入（改为按文档手动修改）
    [switch]$SkipManifestPatch,
    # 跳过 rustup target add（已安装时使用）
    [switch]$SkipTargetsInstall
)

$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $true  # PowerShell 7+；5.1 下由 $LASTEXITCODE 检查兜底

function Write-Step([string]$msg) { Write-Host "`n==> $msg" -ForegroundColor Cyan }
function Write-Ok([string]$msg) { Write-Host "    OK: $msg" -ForegroundColor Green }
function Write-Warn([string]$msg) { Write-Host "    WARN: $msg" -ForegroundColor Yellow }
function Assert-LastExit([string]$what) {
    if ($LASTEXITCODE -ne 0) { throw "$what 失败（exit=$LASTEXITCODE）" }
}

$root = Split-Path -Parent $PSScriptRoot   # 仓库根目录
Set-Location $root

Write-Step "前置检查"
# --- pnpm / node ---
if (-not (Get-Command pnpm -ErrorAction SilentlyContinue)) {
    throw '未找到 pnpm。请先安装 Node.js 22+ 与 pnpm（corepack enable / npm i -g pnpm）。'
}
Write-Ok "pnpm: $((Get-Command pnpm).Source)"

# --- rust / rustup ---
if (-not (Get-Command cargo -ErrorAction SilentlyContinue) -or -not (Get-Command rustup -ErrorAction SilentlyContinue)) {
    throw '未找到 cargo/rustup。请先安装 Rust（https://rustup.rs）。'
}
Write-Ok "cargo: $((Get-Command cargo).Source)"

# --- Rust Android target ---
if (-not $SkipTargetsInstall) {
    Write-Step "安装 Rust Android target（aarch64-linux-android）"
    rustup target add aarch64-linux-android
    Assert-LastExit 'rustup target add'
    Write-Ok 'aarch64-linux-android'
} else {
    Write-Warn '已跳过 rustup target 安装'
}

# --- JDK ---
$javaOk = $false
if ($env:JAVA_HOME -and (Test-Path (Join-Path $env:JAVA_HOME 'bin\java.exe'))) { $javaOk = $true }
if (-not $javaOk -and (Get-Command java -ErrorAction SilentlyContinue)) { $javaOk = $true }
if (-not $javaOk) {
    Write-Warn '未找到 JDK 17+。请设置 JAVA_HOME 或安装 Android Studio（自带 JBR）。CI 无需本机 JDK。'
} else {
    Write-Ok "JAVA_HOME=$env:JAVA_HOME"
}

# --- Android SDK / NDK ---
$androidHome = $env:ANDROID_HOME
if (-not $androidHome) { $androidHome = $env:ANDROID_SDK_ROOT }
if (-not $androidHome -or -not (Test-Path $androidHome)) {
    Write-Warn '未找到 Android SDK（ANDROID_HOME 未设置或路径不存在）。'
    Write-Warn '本机无法完成 android init 的依赖安装；可改用 GitHub Actions（android-job.yml）。'
    Write-Warn '如已安装 SDK：$env:ANDROID_HOME = "C:\Android\Sdk" 后重试。'
} else {
    Write-Ok "ANDROID_HOME=$androidHome"
    $ndkOk = $false
    if ($env:NDK_HOME -and (Test-Path $env:NDK_HOME)) { $ndkOk = $true }
    elseif ($env:ANDROID_NDK_HOME -and (Test-Path $env:ANDROID_NDK_HOME)) { $ndkOk = $true }
    elseif (Test-Path (Join-Path $androidHome 'ndk')) { $ndkOk = $true }
    if (-not $ndkOk) {
        Write-Warn '未找到 NDK（tauri 2.11 要求 NDK 29.0.13846066，可通过 sdkmanager 安装）。'
    } else {
        Write-Ok 'NDK 已就绪'
    }
}

# --- 依赖安装（首次需要） ---
if (-not (Test-Path (Join-Path $root 'node_modules'))) {
    Write-Step "安装前端依赖 pnpm install"
    pnpm install
    Assert-LastExit 'pnpm install'
    Write-Ok 'node_modules'
} else {
    Write-Ok 'node_modules 已存在，跳过 pnpm install'
}

# --- tauri android init ---
Write-Step "生成 Android 工程（tauri android init --ci）"
if (-not (Test-Path (Join-Path $root 'src-tauri\gen\android'))) {
    pnpm tauri android init --ci
    Assert-LastExit 'tauri android init'
    Write-Ok 'src-tauri/gen/android 已生成'
} else {
    Write-Warn 'src-tauri/gen/android 已存在，跳过 init（如需重新生成请删除该目录后重跑）'
}

# --- 注入 Kotlin 桥接 ---
Write-Step "注入 Kotlin 桥接 FeatherViewPlugin.kt"
$packagePath = 'com/featherview/app'   # 与 tauri.conf.json identifier 一致
$kotlinSrc = Join-Path $PSScriptRoot 'android\FeatherViewPlugin.kt'
$genAndroid = Join-Path $root 'src-tauri\gen\android'
$kotlinDest = Join-Path $genAndroid "app\src\main\java\$packagePath\FeatherViewPlugin.kt"
if (Test-Path $kotlinSrc) {
    if (Test-Path $kotlinDest) {
        Write-Warn "FeatherViewPlugin.kt 已存在（$kotlinDest），跳过覆盖（如需更新请手动替换）"
    } else {
        $destDir = Split-Path -Parent $kotlinDest
        New-Item -ItemType Directory -Force -Path $destDir | Out-Null
        Copy-Item $kotlinSrc $kotlinDest -Force
        Write-Ok "已拷贝到 $kotlinDest"
    }
} else {
    Write-Warn "未找到 $kotlinSrc（跳过；不影响构建，Android 原生读取能力降级）"
}

# --- AndroidManifest VIEW intent-filter 注入（幂等） ---
if (-not $SkipManifestPatch) {
    Write-Step "注入 AndroidManifest VIEW intent-filter（打开方式 / 文件关联）"
    $manifest = Join-Path $genAndroid 'app\src\main\AndroidManifest.xml'
    if (-not (Test-Path $manifest)) {
        Write-Warn "未找到 $manifest（init 未完成？），跳过注入"
    } else {
        $xml = Get-Content $manifest -Raw -Encoding UTF8
        if ($xml -match 'android.intent.action.VIEW') {
            Write-Warn '已存在 VIEW intent-filter，跳过'
        } elseif ($xml -match '<activity[\s\S]*?</activity>') {
            $viewFilter = @'
            <!-- FeatherView: 以"打开方式"打开文件（Android SAF content://） -->
            <intent-filter>
                <action android:name="android.intent.action.VIEW" />
                <category android:name="android.intent.category.DEFAULT" />
                <category android:name="android.intent.category.BROWSABLE" />
                <data android:mimeType="text/markdown" />
                <data android:mimeType="text/plain" />
                <data android:mimeType="text/*" />
                <data android:mimeType="application/json" />
                <data android:mimeType="application/x-yaml" />
                <data android:mimeType="application/toml" />
            </intent-filter>

'@
            $xml = $xml -replace '(</activity>)', "$viewFilter`$1"
            Set-Content -Path $manifest -Value $xml -Encoding UTF8 -NoNewline
            Write-Ok 'VIEW intent-filter 已注入（生成文件为 tauri 模板产物，重跑 init 前请备份）'
        } else {
            Write-Warn '未匹配到 <activity> 块，未注入；请按 docs/architecture/android-manifest.md 手动修改'
        }
    }
} else {
    Write-Warn '已跳过 Manifest 补丁（-SkipManifestPatch）'
}

# --- 完成 ---
Write-Step "完成"
Write-Host @"

下一步：
  1) 确认环境变量（PowerShell 会话）：
       `$env:ANDROID_HOME = "C:\Android\Sdk"`        # 指向 SDK
       `$env:NDK_HOME     = "C:\Android\Sdk\ndk\29.0.13846066"`
  2) 构建 Debug APK：
       pnpm tauri android build --debug --target aarch64 --apk
     产物：src-tauri\gen\android\app\build\outputs\apk\universal\debug\app-universal-debug.apk
  3) 安装到设备 / 模拟器：
       adb install -r src-tauri\gen\android\app\build\outputs\apk\universal\debug\app-universal-debug.apk

说明：
  - gen/android 为生成目录（tauri android init 产物 + 本脚本注入），建议加入 .gitignore
    （CI 每次 init 生成，保证与 tauri 版本一致）。
  - tauri android init 不会覆盖已存在的 FeatherViewPlugin.kt（模板渲染仅创建不存在的文件）。
  - AndroidManifest 为生成文件；重跑 init 前请备份本脚本的注入。
  - 无 Android SDK 的本机：全部步骤由 GitHub Actions 的 android-job.yml 完成。
"@ -ForegroundColor Yellow