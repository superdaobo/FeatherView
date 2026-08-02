# unregister.ps1 —— 卸载 LanxiaPreviewHandler（HKCU）
#
# 用法：
#   powershell -ExecutionPolicy Bypass -File unregister.ps1
#
# 说明：
# - 仅删除本组件写入的键（CLSID 键 + 各扩展名 shellex 下的本 CLSID 子键），
#   不删除扩展名本身、不触碰其他软件的 shellex 条目、不修改 HKLM。
param()

$ErrorActionPreference = 'Continue'   # 键不存在时继续

$Clsid = '{8895b1c6-b41f-4c1c-a562-0d564250836f}'
$Extensions = @('md', 'markdown', 'mdown', 'txt', 'log', 'json', 'yaml', 'yml', 'toml', 'csv')

# 1. 删除各扩展名 shellex 下的本 CLSID 子键
foreach ($ext in $Extensions) {
    $shellex = "HKCU:\Software\Classes\.$ext\shellex\$Clsid"
    if (Test-Path $shellex) {
        Remove-Item -Path $shellex -Recurse -Force
        Write-Host "[ok] 已删除 .$ext 的预览处理注册"
    }
}

# 2. 删除 CLSID 键（仅当属于本组件：默认值匹配 'Lanxia Preview Handler' 或 InprocServer32 指向 Lanxia）
$clsidKey = "HKCU:\Software\Classes\CLSID\$Clsid"
if (Test-Path $clsidKey) {
    $def = (Get-ItemProperty -Path $clsidKey -Name '(default)' -ErrorAction SilentlyContinue).'(default)'
    $inproc = (Get-ItemProperty -Path "$clsidKey\InprocServer32" -Name '(default)' -ErrorAction SilentlyContinue).'(default)'
    $isOurs = ($def -eq 'Lanxia Preview Handler') -or ($inproc -like '*Lanxia*')
    if ($isOurs) {
        Remove-Item -Path $clsidKey -Recurse -Force
        Write-Host '[ok] 已删除 CLSID 注册'
    } else {
        Write-Host '[warn] CLSID 键存在但不属于本组件，未删除（请人工检查）' -ForegroundColor Yellow
    }
}

Write-Host '[ok] 卸载完成。'