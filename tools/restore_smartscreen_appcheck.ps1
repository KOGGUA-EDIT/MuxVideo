# 恢复 SmartScreen「检查应用和文件」到备份时的状态。
# 用法（需管理员）：
#   pwsh -NoProfile -ExecutionPolicy Bypass -File .\restore_smartscreen_appcheck.ps1
$ErrorActionPreference = 'Stop'
$outDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$backupPath = Join-Path $outDir 'smartscreen-appcheck-backup.json'

if (-not (Test-Path $backupPath)) {
    Write-Host "找不到备份: $backupPath"
    exit 1
}

$backup = Get-Content -Raw -Path $backupPath | ConvertFrom-Json
foreach ($item in @($backup)) {
    if ($item.Existed) {
        if (-not (Test-Path $item.Path)) { New-Item -Path $item.Path -Force | Out-Null }
        New-ItemProperty -Path $item.Path -Name $item.Name -PropertyType DWord -Value ([int]$item.Value) -Force | Out-Null
        Write-Host ("恢复 {0}\{1} = {2}" -f $item.Path, $item.Name, $item.Value)
    } else {
        Remove-ItemProperty -Path $item.Path -Name $item.Name -ErrorAction SilentlyContinue
        Write-Host ("删除 {0}\{1}（原本不存在）" -f $item.Path, $item.Name)
    }
}
Write-Host 'RESULT: OK - 已恢复。'
