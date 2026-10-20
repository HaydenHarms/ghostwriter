<#
.SYNOPSIS
  Build and package Ghostwriter (this fork, docs/companion.md) for Windows.

.DESCRIPTION
  Produces, in $env:DIST (default: dist/release):
    ghostwriter-<version>-windows-<arch>.msi            per-machine installer (WiX v5, ghostwriter.wxs)
    ghostwriter-<version>-windows-<arch>-portable.zip   the three programs, unzip and run

  The programs are installed as ghostwriter.exe (the word processor), ghostwriter-cli.exe and
  ghostwriter-companion.exe. Unsigned: Windows SmartScreen warns on first launch.

  Needs what package.ps1 needs: Rust (MSVC toolchain + the target) and WiX v5 with its UI and Util
  extensions.

.EXAMPLE
  pwsh packaging/windows/ghostwriter-package.ps1 -Arch x64
#>
param(
  [ValidateSet('x64', 'arm64')] [string] $Arch = 'x64'
)
$ErrorActionPreference = 'Stop'
$Root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

function Invoke-Native([string] $What, [scriptblock] $Block) {
  Write-Output "==> $What"
  & $Block
  if ($LASTEXITCODE -ne 0) { throw "$What failed with exit code $LASTEXITCODE" }
}

# The version lives in one place: [workspace.package] version in the root Cargo.toml.
$Version = $null
$inPkg = $false
foreach ($line in Get-Content (Join-Path $Root 'Cargo.toml')) {
  if ($line -match '^\s*\[') { $inPkg = ($line.Trim() -eq '[workspace.package]'); continue }
  if ($inPkg -and $line -match '^\s*version\s*=\s*"([^"]+)"') { $Version = $Matches[1]; break }
}
if (-not $Version) { throw 'could not read [workspace.package] version from Cargo.toml' }
$MsiVersion = ($Version -split '-')[0]

$Target = switch ($Arch) { 'x64' { 'x86_64-pc-windows-msvc' } 'arm64' { 'aarch64-pc-windows-msvc' } }
$Dist = if ($env:DIST) { $env:DIST } else { Join-Path $Root 'dist\release' }
$TargetDir = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Root 'target' }
New-Item -ItemType Directory -Force -Path $Dist | Out-Null

if (-not $env:WORDCRAFT_BUILD_SHA) { $env:WORDCRAFT_BUILD_SHA = (git -C $Root rev-parse HEAD 2>$null) }
if (-not $env:WORDCRAFT_BUILD_DATE) { $env:WORDCRAFT_BUILD_DATE = (Get-Date).ToUniversalTime().ToString('yyyy-MM-dd') }

Write-Output "Ghostwriter $Version for Windows $Arch ($Target)"

# Static CRT: no VC++ redistributable needed.
$flagVar = 'CARGO_TARGET_' + ($Target.ToUpper() -replace '-', '_') + '_RUSTFLAGS'
[Environment]::SetEnvironmentVariable($flagVar, '-C target-feature=+crt-static')
$env:WORDCRAFT_REQUIRE_WINRES = '1'
Invoke-Native "cargo build ($Target)" { cargo build --release --locked -p wordcraft -p wordcraft-cli -p wordcraft-companion --target $Target }

$Bin = Join-Path $TargetDir "$Target\release"
$Stage = Join-Path $TargetDir "ghostwriter-package\$Arch"
Remove-Item -Recurse -Force $Stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $Stage | Out-Null

# Check each PE header (machine must match -Arch; 2 = GUI, 3 = console) and stage it renamed.
function Get-PeHeader([string] $Path) {
  $bytes = [System.IO.File]::ReadAllBytes($Path)
  $pe = [BitConverter]::ToInt32($bytes, 0x3C)
  return @{ Machine = [BitConverter]::ToUInt16($bytes, $pe + 4); Subsystem = [BitConverter]::ToUInt16($bytes, $pe + 0x5C) }
}
$Machine = switch ($Arch) { 'x64' { 0x8664 } 'arm64' { 0xAA64 } }
foreach ($exe in @(@('wordcraft.exe', 'ghostwriter.exe', 2), @('wordcraft-cli.exe', 'ghostwriter-cli.exe', 3), @('wordcraft-companion.exe', 'ghostwriter-companion.exe', 3))) {
  $h = Get-PeHeader (Join-Path $Bin $exe[0])
  if ($h.Machine -ne $Machine) { throw "$($exe[0]) is for machine 0x$('{0:X}' -f $h.Machine), expected 0x$('{0:X}' -f $Machine) ($Arch)" }
  if ($h.Subsystem -ne $exe[2]) { throw "$($exe[0]) has PE subsystem $($h.Subsystem), expected $($exe[2])" }
  Copy-Item (Join-Path $Bin $exe[0]) (Join-Path $Stage $exe[1])
  Write-Output "ok $($exe[1]): $Arch, PE subsystem $($h.Subsystem)"
}

# ---- MSI ---------------------------------------------------------------------------------------
# The welcome page shows the license, built from LICENSE-MIT and LICENSE-APACHE (see package.ps1).
function ConvertTo-RtfText([string] $Text) {
  $sb = [System.Text.StringBuilder]::new()
  foreach ($ch in $Text.ToCharArray()) {
    $c = [int] $ch
    if ($c -eq 0x5C -or $c -eq 0x7B -or $c -eq 0x7D) { [void] $sb.Append('\').Append($ch) }
    elseif ($c -gt 127) { [void] $sb.Append("\u$(if ($c -gt 32767) { $c - 65536 } else { $c })?") }
    else { [void] $sb.Append($ch) }
  }
  return $sb.ToString()
}
$paragraphs = [System.Collections.Generic.List[string]]::new()
$paragraphs.Add('\b Ghostwriter\b0')
$paragraphs.Add((ConvertTo-RtfText 'Ghostwriter is based on WordCraft by the ArtCraft team and the WordCraft contributors, and is dual-licensed under the MIT License or the Apache License, Version 2.0, at your option. Both license texts follow. Bundled fonts and other third-party material keep their own open licenses, listed in NOTICE and ATTRIBUTION.md at https://github.com/storytold/wordcraft.'))
foreach ($file in 'LICENSE-MIT', 'LICENSE-APACHE') {
  $title = $true
  foreach ($para in ((Get-Content -Raw (Join-Path $Root $file)) -split '\r?\n\s*\r?\n')) {
    $joined = (($para -split '\r?\n') | ForEach-Object { $_.Trim() } | Where-Object { $_ }) -join ' '
    if (-not $joined) { continue }
    $paragraphs.Add($(if ($title) { "\b $(ConvertTo-RtfText $joined)\b0" } else { ConvertTo-RtfText $joined }))
    $title = $false
  }
}
$LicenseRtf = Join-Path $Stage 'license.rtf'
$rtf = '{\rtf1\ansi\ansicpg1252\deff0{\fonttbl{\f0\fswiss\fcharset0 Tahoma;}}\viewkind4\uc1\pard\sa120\f0\fs16 ' +
  ($paragraphs -join "\par`r`n") + "\par`r`n}`r`n"
[System.IO.File]::WriteAllText($LicenseRtf, $rtf, [System.Text.Encoding]::ASCII)

$Msi = Join-Path $Dist "ghostwriter-$Version-windows-$Arch.msi"
Invoke-Native 'wix build' {
  wix build (Join-Path $PSScriptRoot 'ghostwriter.wxs') -arch $Arch `
    -ext WixToolset.UI.wixext -ext WixToolset.Util.wixext `
    -d "Version=$MsiVersion" -d "BinDir=$Stage" -d "IconPath=$(Join-Path $Root 'assets\app-icon\wordcraft.ico')" `
    -d "LicenseRtf=$LicenseRtf" `
    -o $Msi
}
Remove-Item -Force -ErrorAction SilentlyContinue ([IO.Path]::ChangeExtension($Msi, '.wixpdb'))

# ---- portable zip ------------------------------------------------------------------------------
$Portable = Join-Path $TargetDir "ghostwriter-package\ghostwriter-$Version-windows-$Arch-portable"
Remove-Item -Recurse -Force $Portable -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force -Path $Portable | Out-Null
Copy-Item (Join-Path $Stage '*.exe') $Portable
foreach ($f in 'README.md', 'LICENSE', 'LICENSE-MIT', 'LICENSE-APACHE', 'NOTICE', 'docs\companion.md') {
  $p = Join-Path $Root $f
  if (Test-Path $p) { Copy-Item $p $Portable }
}
if ($env:CRAFT_FONTS_DIR) {
  foreach ($ofl in Get-ChildItem -Path (Join-Path $env:CRAFT_FONTS_DIR 'fonts\*\OFL.txt') -ErrorAction SilentlyContinue) {
    Copy-Item $ofl.FullName (Join-Path $Portable "OFL-$($ofl.Directory.Name).txt")
  }
}
$Zip = Join-Path $Dist "ghostwriter-$Version-windows-$Arch-portable.zip"
Remove-Item -Force $Zip -ErrorAction SilentlyContinue
Compress-Archive -Path $Portable -DestinationPath $Zip

# Smoke-test the CLI and the companion when this machine can run them.
$HostArch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString().ToLowerInvariant()
if ($Arch -ne 'arm64' -or $HostArch -eq 'arm64') {
  Invoke-Native 'ghostwriter-cli --version' { & (Join-Path $Stage 'ghostwriter-cli.exe') --version }
  Invoke-Native 'ghostwriter-companion --help' { & (Join-Path $Stage 'ghostwriter-companion.exe') --help }
}
Get-Item $Msi, $Zip | Format-Table Name, Length
