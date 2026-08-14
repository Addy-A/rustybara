<#
.SYNOPSIS
    Build the rbara Windows installer end-to-end.

.DESCRIPTION
    1. Reads the version from rbara/Cargo.toml.
    2. Builds rbara in release mode with the static MSVC CRT
       (so the installer does not need to ship the VC++ redistributable).
    3. Downloads pdfium-win-x64 at the pinned chromium build number
       and extracts pdfium.dll into installer/windows/assets/.
    4. Runs Inno Setup (iscc.exe) to produce
       installer/windows/dist/rbara-setup-<version>-x64.exe.

.PARAMETER PdfiumChromium
    Chromium build number to fetch from bblanchon/pdfium-binaries.
    Default: 7776 (PDFium 148.0.7776.0).

.PARAMETER ForceDownload
    Re-download pdfium even if assets/pdfium.dll already exists.

.EXAMPLE
    .\build-installer.ps1
    .\build-installer.ps1 -PdfiumChromium 7811
#>
[CmdletBinding()]
param(
    [string]$PdfiumChromium = '7776',
    [switch]$ForceDownload
)

$ErrorActionPreference = 'Stop'

# --- Resolve paths -----------------------------------------------------------
$ScriptDir   = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot    = Resolve-Path (Join-Path $ScriptDir '..\..')
$AssetsDir   = Join-Path $ScriptDir 'assets'
$BuildDir    = Join-Path $ScriptDir 'build'
$DistDir     = Join-Path $ScriptDir 'dist'
$IssScript   = Join-Path $ScriptDir 'rbara.iss'
$CargoToml   = Join-Path $RepoRoot 'rbara\Cargo.toml'

New-Item -ItemType Directory -Force -Path $AssetsDir, $BuildDir, $DistDir | Out-Null

# --- 1. Read version ---------------------------------------------------------
Write-Host '==> Reading rbara version from Cargo.toml' -ForegroundColor Cyan
$cargoLines = Get-Content $CargoToml
$versionLine = $cargoLines | Select-String -Pattern '^\s*version\s*=\s*"([^"]+)"' | Select-Object -First 1
if (-not $versionLine) { throw "Could not find version in $CargoToml" }
$AppVersion = $versionLine.Matches[0].Groups[1].Value
$FileVersion = $AppVersion -replace '-', '.'
if ($FileVersion -notmatch '^\d+(\.\d+){0,3}$') {
    throw "Cannot convert rbara version '$AppVersion' to a numeric Windows file version."
}
Write-Host "    rbara version: $AppVersion"
Write-Host "    Windows file version: $FileVersion"
Write-Host "    pdfium chromium build: $PdfiumChromium"

# --- 2. Build rbara release with static CRT ----------------------------------
Write-Host '==> Building rbara (release, static CRT)' -ForegroundColor Cyan
$env:RUSTFLAGS = '-C target-feature=+crt-static'
try {
    Push-Location $RepoRoot
    cmd /c "cargo build --release -p rbara --target x86_64-pc-windows-msvc 2>&1"
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed (exit $LASTEXITCODE)" }
} finally {
    Pop-Location
    Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue
}

# Cargo with explicit --target writes to target/<triple>/release/.
# rbara.iss reads ..\..\target\release\rbara.exe, so copy/promote.
$BuiltExe = Join-Path $RepoRoot 'target\x86_64-pc-windows-msvc\release\rbara.exe'
$StagedExe = Join-Path $RepoRoot 'target\release\rbara.exe'
if (-not (Test-Path $BuiltExe)) { throw "Expected build output not found: $BuiltExe" }
Copy-Item -Force $BuiltExe $StagedExe
Write-Host "    Staged: $StagedExe"

# --- 3. Fetch pdfium ---------------------------------------------------------
$PdfiumDll = Join-Path $AssetsDir 'pdfium.dll'
if ($ForceDownload -or -not (Test-Path $PdfiumDll)) {
    Write-Host "==> Downloading pdfium chromium/$PdfiumChromium (win-x64)" -ForegroundColor Cyan
    $tgzPath = Join-Path $AssetsDir 'pdfium.tgz'
    $url = "https://github.com/bblanchon/pdfium-binaries/releases/download/chromium%2F$PdfiumChromium/pdfium-win-x64.tgz"
    Write-Host "    URL: $url"
    Invoke-WebRequest -Uri $url -OutFile $tgzPath -UseBasicParsing

    Write-Host '    Extracting bin/pdfium.dll'
    $extractDir = Join-Path $BuildDir 'pdfium'
    if (Test-Path $extractDir) { Remove-Item -Recurse -Force $extractDir }
    New-Item -ItemType Directory -Path $extractDir | Out-Null

    # tar is shipped with Windows 10 1803+. Use it for .tgz extraction.
    cmd /c "tar -xzf `"$tgzPath`" -C `"$extractDir`" 2>&1"
    if ($LASTEXITCODE -ne 0) { throw "tar extraction failed (exit $LASTEXITCODE)" }

    $extractedDll = Join-Path $extractDir 'bin\pdfium.dll'
    if (-not (Test-Path $extractedDll)) { throw "pdfium.dll not found inside archive at $extractedDll" }
    Copy-Item -Force $extractedDll $PdfiumDll
    Remove-Item -Force $tgzPath
    Write-Host "    Saved: $PdfiumDll"
} else {
    Write-Host '==> Reusing existing pdfium.dll (use -ForceDownload to refresh)' -ForegroundColor Yellow
}

# --- 4. Locate Inno Setup ----------------------------------------------------
Write-Host '==> Locating Inno Setup compiler (iscc.exe)' -ForegroundColor Cyan
$iscc = (Get-Command iscc.exe -ErrorAction SilentlyContinue).Source
if (-not $iscc) {
    $candidates = @(
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
        "${env:ProgramFiles}\Inno Setup 6\ISCC.exe"
    )
    $iscc = $candidates | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $iscc) {
    throw "Inno Setup 6 not found. Install from https://jrsoftware.org/isdl.php and re-run."
}
Write-Host "    iscc: $iscc"

# --- 5. Compile installer ----------------------------------------------------
Write-Host '==> Compiling installer' -ForegroundColor Cyan
cmd /c "`"$iscc`" `"/DAppVersion=$AppVersion`" `"/DFileVersion=$FileVersion`" `"/DPdfiumChromium=$PdfiumChromium`" `"$IssScript`" 2>&1"
if ($LASTEXITCODE -ne 0) { throw "iscc failed (exit $LASTEXITCODE)" }

$Output = Join-Path $DistDir "rbara-setup-$AppVersion-x64.exe"
if (Test-Path $Output) {
    Write-Host ''
    Write-Host "[OK] Built: $Output" -ForegroundColor Green
    Write-Host ('     Size: {0:N2} MB' -f ((Get-Item $Output).Length / 1MB))
} else {
    Write-Warning "Installer expected at $Output but not found."
}
