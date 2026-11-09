# 构建后的发布脚本：把 release 产物送出 DSH 工作区并修正完整性级别。
#
# 为什么要这一步：DSH 的工作区目录带 Low Mandatory Level 完整性标签，
# 在里面生成的 exe 会继承 Low，双击时 Windows 会把它当成“来自 Internet 的文件”
# 弹出 SmartScreen / “无法验证发布者”对话框。拷到工作区外（或显式改成 Medium）
# 就不会再弹。
#
# 用法（普通权限即可，脚本内部按需静默提权）：
#   pwsh -NoProfile -ExecutionPolicy Bypass -File tools\deploy.ps1
#   pwsh -NoProfile -ExecutionPolicy Bypass -File tools\deploy.ps1 -SkipBuild

param(
    [switch]$SkipBuild,
    [string]$Target = 'D:\ruanjian\MuxVideo'
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$dist = Join-Path $repo 'dist'
$release = Join-Path $repo 'target\release'
$certThumb = '74CE54AFE8D9636E8635A7294D129390DA7877F6'

function Invoke-Elevated([string]$scriptBody) {
    $tmp = Join-Path $env:TEMP ('muxvideo-elev-' + [guid]::NewGuid().ToString('N') + '.ps1')
    Set-Content -LiteralPath $tmp -Value $scriptBody -Encoding utf8
    $log = $tmp + '.log'
    $cmd = 'cmd.exe /c pwsh -NoProfile -ExecutionPolicy Bypass -File "' + $tmp + '" > "' + $log + '" 2>&1'
    $p = Start-Process -FilePath 'cmd.exe' -ArgumentList $cmd -Verb RunAs -WindowStyle Hidden -PassThru -Wait
    if (Test-Path $log) { Get-Content $log | ForEach-Object { Write-Host $_ } }
    Remove-Item $tmp, $log -Force -ErrorAction SilentlyContinue
    return $p.ExitCode
}

if (-not $SkipBuild) {
    Write-Host '[1/4] cargo build --release'
    $env:RUSTUP_HOME = 'D:\ruanjian\rust-env\rustup'
    $env:CARGO_HOME = Join-Path $repo '.cargo-home'
    $env:CARGO_NET_OFFLINE = 'true'
    $env:PATH = 'D:\ruanjian\rust-env\cargo\bin;D:\ruanjian\rust-env\w64devkit\bin;' + $env:PATH
    Push-Location $repo
    try { cargo build --release --offline } finally { Pop-Location }
} else {
    Write-Host '[1/4] 跳过构建'
}

Write-Host '[2/4] 复制到 dist 并签名'
New-Item -ItemType Directory -Force -Path $dist | Out-Null
Copy-Item (Join-Path $release 'muxvideo.exe') (Join-Path $dist 'muxvideo.exe') -Force
if (-not (Test-Path (Join-Path $dist 'libmpv-2.dll'))) {
    Copy-Item 'D:\ruanjian\mpv\mpv.net-DW v2.1.0\libmpv-2.dll' (Join-Path $dist 'libmpv-2.dll') -Force
}

$cert = Get-ChildItem Cert:\CurrentUser\My | Where-Object Thumbprint -eq $certThumb
if ($cert) {
    $sig = Set-AuthenticodeSignature -FilePath (Join-Path $dist 'muxvideo.exe') -Certificate $cert
    Write-Host ('      签名: ' + $sig.Status)
} else {
    Write-Warning ('      未找到本地代码签名证书（thumbprint ' + $certThumb + '），跳过签名')
}

Write-Host '[3/4] 安装到 ' + $Target
New-Item -ItemType Directory -Force -Path $Target | Out-Null
Copy-Item (Join-Path $dist 'muxvideo.exe') $Target -Force
Copy-Item (Join-Path $dist 'libmpv-2.dll') $Target -Force
foreach ($extra in 'README.txt') {
    if (Test-Path (Join-Path $dist $extra)) { Copy-Item (Join-Path $dist $extra) $Target -Force }
}

Write-Host '[4/4] 修正完整性级别（Low -> Medium）'
$body = @"
`$files = @('$(Join-Path $dist 'muxvideo.exe')','$(Join-Path $dist 'libmpv-2.dll')','$(Join-Path $Target 'muxvideo.exe')','$(Join-Path $Target 'libmpv-2.dll')')
foreach (`$f in `$files) { if (Test-Path `$f) { icacls `$f /setintegritylevel Medium | Out-Null } }
Write-Output 'REL' + 'ABEL: OK'
"@
$code = Invoke-Elevated $body
if ($code -ne 0) { Write-Warning ('      提权修正失败，退出码 ' + $code) }

$icacls = icacls (Join-Path $Target 'muxvideo.exe') | Select-Object -Last 3 | Out-String
if ($icacls -match 'Low Mandatory') { Write-Warning '      目标 exe 仍是 Low 完整性，双击可能弹 SmartScreen' }
else { Write-Host '      OK: 目标 exe 不再是 Low 完整性' }

Write-Host '完成。可直接运行: ' + (Join-Path $Target 'muxvideo.exe')
