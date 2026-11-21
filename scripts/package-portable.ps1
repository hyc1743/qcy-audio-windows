<#
.SYNOPSIS
    Verifies and packages a portable Windows x64 ZIP of QCY Desktop from target/release/.
    Runs `qcy-desktop.exe --smoke-test` to ensure embedded frontend assets (`tauri.localhost`)
    are bundled before creating the archive, and automatically includes WebView2Loader.dll
    when built with x86_64-pc-windows-gnu.
#>
[CmdletBinding()]
param(
    [string]$Profile = "release",
    [string]$OutDir = "dist-portable",
    [switch]$SkipSmokeTest
)

$ErrorActionPreference = "Stop"
$RepoRoot = Resolve-Path (Join-Path $PSScriptRoot "..")
$TargetDir = Join-Path $RepoRoot "target\$Profile"
$ExePath = Join-Path $TargetDir "qcy-desktop.exe"

if (-not (Test-Path $ExePath)) {
    throw "Executable not found at '$ExePath'. Run 'npm run build' and 'cargo build --release -p qcy-desktop --features custom-protocol' (or 'npm run tauri -- build') first."
}

if (-not $SkipSmokeTest) {
    $stdoutFile = [System.IO.Path]::GetTempFileName()
    $stderrFile = [System.IO.Path]::GetTempFileName()
    try {
        $proc = Start-Process -FilePath $ExePath -ArgumentList "--smoke-test" -Wait -PassThru `
            -RedirectStandardOutput $stdoutFile -RedirectStandardError $stderrFile
        $stdoutText = (Get-Content -Raw -Path $stdoutFile -ErrorAction SilentlyContinue)
        $stderrText = (Get-Content -Raw -Path $stderrFile -ErrorAction SilentlyContinue)
        if ($stdoutText) { Write-Host $stdoutText.Trim() }

        if ($proc.ExitCode -ne 0) {
            if ($stderrText) { Write-Error $stderrText.Trim() -ErrorAction Continue }
            throw "Portable smoke-test failed with exit code $($proc.ExitCode). Rebuild with '--features custom-protocol' or 'npm run tauri -- build'."
        }
        if ($stdoutText -notmatch "SMOKE_TEST_OK:") {
            throw "Portable smoke-test did not emit SMOKE_TEST_OK."
        }
        if ($stdoutText -match "localhost:1420" -or $stdoutText -notmatch "dev_build=false" -or $stdoutText -notmatch "index_has_script=true") {
            throw "Binary is missing embedded frontend assets (devUrl detected). Rebuild with 'cargo build --release -p qcy-desktop --features custom-protocol' or 'npm run tauri -- build'."
        }
        if ($stdoutText -notmatch "simulator_enabled=false" -or $stdoutText -notmatch "connection_state=disconnected" -or $stdoutText -notmatch "active_device=none") {
            throw "Binary did not start in clean disconnected real-Bluetooth default state."
        }
        if ($stdoutText -notmatch "main_url=https?://tauri\.localhost" -and $stdoutText -notmatch "main_url=tauri://localhost") {
            throw "Binary main_url is not serving embedded Tauri custom-protocol assets."
        }
    } finally {
        Remove-Item -Force -ErrorAction SilentlyContinue $stdoutFile, $stderrFile
    }
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
