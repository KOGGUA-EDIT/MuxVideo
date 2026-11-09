# 恢复 SmartScreen 相关注册表：还原 AppHost 三处原值，并删除本次新增的 Policy 值。
$ErrorActionPreference = 'Stop'
$outDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$backupPath = Join-Path $outDir 'smartscreen-appcheck-backup.json'

if (Test-Path $backupPath) {
    $backup = Get-Content -Raw -Path $backupPath | ConvertFrom-Json
    foreach ($item in @($backup)) {
        if ($item.Existed) {
            if (-not (Test-Path $item.Path)) { New-Item -Path $item.Path -Force | Out-Null }
            New-ItemProperty -Path $item.Path -Name $item.Name -PropertyType DWord -Value ([int]$item.Value) -Force | Out-Null
            Write-Output ("restore {0}\{1} = {2}" -f $item.Path, $item.Name, $item.Value)
        } else {
            Remove-ItemProperty -Path $item.Path -Name $item.Name -ErrorAction SilentlyContinue
            Write-Output ("delete {0}\{1} (absent before)" -f $item.Path, $item.Name)
        }
    }
} else {
    Write-Output 'backup missing'
}

$policyKey = 'HKLM:\SOFTWARE\Policies\Microsoft\Windows\System'
foreach ($name in 'EnableSmartScreen', 'ShellSmartScreenLevel') {
    Remove-ItemProperty -Path $policyKey -Name $name -ErrorAction SilentlyContinue
    Write-Output ("removed policy {0}" -f $name)
}

Get-Process CHXSmartScreen -ErrorAction SilentlyContinue | Stop-Process -Force
Write-Output 'RESULT: RESTORED'
