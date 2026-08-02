# register.ps1 —— 注册 LanxiaPreviewHandler（HKCU，无需管理员）
#
# 用法：
#   powershell -ExecutionPolicy Bypass -File register.ps1
#   powershell -ExecutionPolicy Bypass -File register.ps1 -DllPath "D:\path\to\LanxiaPreviewHandler.dll"
#
# 说明：
# - CLSID 固定为 {8895b1c6-b41f-4c1c-a562-0d564250836f}（架构文档登记值，P0 合同）
# - 注册 HKCU\Software\Classes\CLSID\{...}（InprocServer32 + ThreadingModel=Apartment）
# - 为 9 个扩展名注册 PreviewHandler（HKCU\Software\Classes\.ext\shellex\{CLSID}）
# - 不修改 HKLM、不修改文件关联的默认打开方式，仅追加预览处理
# - 注册后需重启 explorer.exe（或注销重登）生效
param(
    [string]$DllPath = ""
)

$ErrorActionPreference = 'Stop'

$Clsid = '{8895b1c6-b41f-4c1c-a562-0d564250836f}'
$Extensions = @('md', 'markdown', 'mdown', 'txt', 'log', 'json', 'yaml', 'yml', 'toml', 'csv')

# 1. 定位 DLL
if (-not $DllPath) {
    # 默认取脚本同目录（构建产物 release 目录）
    $scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
    $candidates = @(
        (Join-Path $scriptDir '..\..\..\target\release\lanxia_preview_handler.dll'),
        (Join-Path $scriptDir '..\..\..\target\debug\lanxia_preview_handler.dll'),
        (Join-Path $scriptDir 'LanxiaPreviewHandler.dll')
    )
    foreach ($c in $candidates) {
        if (Test-Path $c) { $DllPath = (Resolve-Path $c).Path; break }
    }
}
if (-not $DllPath -or -not (Test-Path $DllPath)) {
    Write-Host '[ERROR] 未找到 LanxiaPreviewHandler.dll，请用 -DllPath 显式指定。' -ForegroundColor Red
    exit 1
}
$DllPath = (Resolve-Path $DllPath).Path
Write-Host "[info] DLL: $DllPath"

# 2. CLSID 注册
$clsidKey = "HKCU:\Software\Classes\CLSID\$Clsid"
New-Item -Path $clsidKey -Force | Out-Null
Set-ItemProperty -Path $clsidKey -Name '(default)' -Value 'Lanxia Preview Handler'
$inproc = "$clsidKey\InprocServer32"
New-Item -Path $inproc -Force | Out-Null
Set-ItemProperty -Path $inproc -Name '(default)' -Value $DllPath
Set-ItemProperty -Path $inproc -Name 'ThreadingModel' -Value 'Apartment'

# 3. 扩展名 PreviewHandler 注册
foreach ($ext in $Extensions) {
    $shellex = "HKCU:\Software\Classes\.$ext\shellex\$Clsid"
    New-Item -Path $shellex -Force | Out-Null
    Set-ItemProperty -Path $shellex -Name '(default)' -Value $Clsid
    Write-Host "[ok] .$ext -> $Clsid"
}

Write-Host '[ok] 注册完成。请重启资源管理器（任务管理器 → 重启 explorer.exe）后验证预览窗格。' -ForegroundColor Green