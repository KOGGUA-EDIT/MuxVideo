param([string]$Tag = "run")

$log = 'D:\暂时\muxvideo-dev\app-debug.log'
if (Test-Path $log) { Remove-Item $log -Force }
Start-Sleep -Milliseconds 150

pwsh -NoProfile -File 'D:\app edit\Muxvideo\tools\real_drag.ps1' -X1 500 -Y1 400 -X2 640 -Y2 400 | Out-Null
Start-Sleep -Milliseconds 500

if (-not (Test-Path $log)) { Write-Output "$Tag : NO LOG"; exit }
$lines = Get-Content $log
$down = ($lines | Where-Object { $_ -match '\[native\] Down' }).Count
$up = ($lines | Where-Object { $_ -match '\[native\] Up' }).Count
$poll = ($lines | Where-Object { $_ -match '\[poll\]' }) -join ' | '
$last = ($lines | Where-Object { $_ -match '\[native\] (Down|Up)' } | Select-Object -Last 2) -join ' ; '
Write-Output "$Tag : down=$down up=$up poll=[$poll] $last"
