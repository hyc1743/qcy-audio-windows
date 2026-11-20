<#
.SYNOPSIS
    Packages a local portable Windows x64 ZIP of QCY Desktop from target/release/.
    Automatically includes WebView2Loader.dll when built with x86_64-pc-windows-gnu.
#>
[CmdletBinding()]
param(
    [string]$Profile = "release",
    [string]$OutDir = "dist-portable"
)

$ErrorActionPreference = "Stop"
$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$TargetDir = Join-Path $RepoRoot "target\$Profile"
$ExePath = Join-Path $TargetDir "qcy-desktop.exe"

if (-not (Test-Path $ExePath)) {
    throw "Executable not found at '$ExePath'. Run 'npm run build' and 'cargo build --release -p qcy-desktop' first."
}

$StageDir = Join-Path $RepoRoot "$OutDir\qcy-desktop-windows-x64"
if (Test-Path $StageDir) { Remove-Item -Recurse -Force $StageDir }
New-Item -ItemType Directory -Force -Path $StageDir | Out-Null

Copy-Item $ExePath -Destination $StageDir

$WebView2Dll = Join-Path $TargetDir "WebView2Loader.dll"
if (Test-Path $WebView2Dll) {
    Copy-Item $WebView2Dll -Destination $StageDir
    Write-Host "Included WebView2Loader.dll (GNU dynamic WebView2 loader)."
} else {
    Write-Host "WebView2Loader.dll not present (MSVC static loader build)."
}

foreach ($doc in @("LICENSE", "README.md")) {
    $docPath = Join-Path $RepoRoot $doc
    if (Test-Path $docPath) { Copy-Item $docPath -Destination $StageDir }
}

$ZipPath = Join-Path $RepoRoot "$OutDir\qcy-desktop-windows-x64-portable.zip"
if (Test-Path $ZipPath) { Remove-Item -Force $ZipPath }
Compress-Archive -Path "$StageDir\*" -DestinationPath $ZipPath

$Hash = (Get-FileHash $ZipPath -Algorithm SHA256).Hash.ToLowerInvariant()
Write-Host "Portable package created: $ZipPath"
Write-Host "SHA-256: $Hash"
