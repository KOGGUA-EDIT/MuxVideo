# 把构建产物设为 Medium 完整性级别，使其不再被 SmartScreen 当成“来自 Internet 的文件”。
# 需要管理员权限（设置完整性标签需要 SeRelabelPrivilege）。
$ErrorActionPreference = 'Continue'
$targets = @(
    'D:\app edit\Muxvideo\dist\muxvideo.exe',
    'D:\app edit\Muxvideo\dist\libmpv-2.dll'
)
foreach ($t in $targets) {
    if (-not (Test-Path $t)) { Write-Output ("skip missing: " + $t); continue }
    icacls $t /setintegritylevel Medium 2>&1 | ForEach-Object { Write-Output $_ }
}
Write-Output 'RESULT: DONE'
