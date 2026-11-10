# Builds the MuxVideo release artifacts:
#   1. release build of the player
#   2. payload staging (exe + libmpv-2.dll + README.txt) for the installer
#   3. setup wizard built with --features payload (self-contained .exe)
#   4. portable zip
#
# Usage: pwsh -NoProfile -File tools\build_setup.ps1 [-SkipPlayer]

param(
    [switch]$SkipPlayer
)

$ErrorActionPreference = 'Stop'

$root = Split-Path -Parent $PSScriptRoot
Set-Location $root

$env:RUSTUP_HOME = 'D:\ruanjian\rust-env\rustup'
$env:CARGO_HOME = Join-Path $root '.cargo-home'
$env:PATH = 'D:\ruanjian\rust-env\cargo\bin;D:\ruanjian\rust-env\w64devkit\bin;' + $env:PATH
$env:CARGO_NET_OFFLINE = 'true'

$version = (Select-String -Path 'Cargo.toml' -Pattern '^version = "(.+)"').Matches[0].Groups[1].Value
Write-Host "MuxVideo $version" -ForegroundColor Cyan

if (-not $SkipPlayer) {
    Write-Host '== release build (player) =='
    cargo build --offline --release --bin muxvideo
    if ($LASTEXITCODE -ne 0) { throw 'player build failed' }
}

$dist = Join-Path $root 'dist'
$payload = Join-Path $root 'assets\payload'
New-Item -ItemType Directory -Force -Path $dist, $payload | Out-Null

Write-Host '== staging payload =='
Copy-Item 'target\release\muxvideo.exe' (Join-Path $payload 'muxvideo.exe') -Force
Copy-Item 'tools\mpv\libmpv-2.dll' (Join-Path $payload 'libmpv-2.dll') -Force
Copy-Item 'dist\README.txt' (Join-Path $payload 'README.txt') -Force

Write-Host '== build setup wizard =='
cargo build --offline --release --bin installer --features payload
if ($LASTEXITCODE -ne 0) { throw 'installer build failed' }

$setup = Join-Path $dist "MuxVideo-Setup-$version.exe"
Copy-Item 'target\release\installer.exe' $setup -Force

Write-Host '== portable zip =='
$portableDir = Join-Path $env:TEMP "muxvideo-portable-$version"
if (Test-Path $portableDir) { Remove-Item $portableDir -Recurse -Force }
New-Item -ItemType Directory -Force -Path $portableDir | Out-Null
Copy-Item 'target\release\muxvideo.exe' $portableDir -Force
Copy-Item 'tools\mpv\libmpv-2.dll' $portableDir -Force
Copy-Item 'dist\README.txt' $portableDir -Force
$portable = Join-Path $dist "MuxVideo-Portable-$version.zip"
if (Test-Path $portable) { Remove-Item $portable -Force }
Compress-Archive -Path (Join-Path $portableDir '*') -DestinationPath $portable -CompressionLevel Optimal

function Format-Size([long]$bytes) {
    if ($bytes -ge 1GB) { '{0:N1} GB' -f ($bytes / 1GB) }
    elseif ($bytes -ge 1MB) { '{0:N1} MB' -f ($bytes / 1MB) }
    else { '{0:N0} KB' -f ($bytes / 1KB) }
}

foreach ($file in @($setup, $portable)) {
    $size = (Get-Item $file).Length
    Write-Host ("{0}  {1}" -f (Split-Path -Leaf $file), (Format-Size $size))
}
Write-Host 'LABEL: OK'
