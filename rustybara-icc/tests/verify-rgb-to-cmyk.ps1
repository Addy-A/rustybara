[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
$crateRoot = Split-Path -Parent $PSScriptRoot
$workspaceRoot = Split-Path -Parent $crateRoot
$fixturePath = Join-Path $PSScriptRoot 'fixtures\rgb-vector-image.pdf'
$artifactDir = Join-Path $workspaceRoot 'target\rgb-to-cmyk-debug'
$outputPath = Join-Path $artifactDir 'rgb-vector-image-cmyk.pdf'
$qdfPath = Join-Path $artifactDir 'rgb-vector-image-cmyk.qdf.pdf'

$qpdf = Get-Command qpdf -ErrorAction Stop
$ripgrep = Get-Command rg -ErrorAction Stop
New-Item -ItemType Directory -Force -Path $artifactDir | Out-Null

Push-Location $workspaceRoot
try {
    & cargo run -q -p rustybara-icc --features bundled-profiles -- `
        $fixturePath $outputPath --from srgb --to USWebCoatedSWOP --intent relative
    if ($LASTEXITCODE -ne 0) {
        throw "rustybara-icc conversion failed with exit code $LASTEXITCODE"
    }

    & $qpdf.Source --check $outputPath
    if ($LASTEXITCODE -ne 0) {
        throw "qpdf structural check failed with exit code $LASTEXITCODE"
    }

    & $qpdf.Source --qdf --object-streams=disable $outputPath $qdfPath
    if ($LASTEXITCODE -ne 0) {
        throw "qpdf QDF decode failed with exit code $LASTEXITCODE"
    }

    Write-Host "QDF output: $qdfPath"
    $rgbPattern = '/DeviceRGB|(^|[[:space:]])(rg|RG)([[:space:]]|$)'
    $remainingRgb = & $ripgrep.Source -a -n -e $rgbPattern -- $qdfPath
    if ($LASTEXITCODE -gt 1) {
        throw "QDF RGB scan failed with exit code $LASTEXITCODE"
    }
    $hasRemainingRgb = $LASTEXITCODE -eq 0

    & cargo test -p rustybara-icc --features bundled-profiles --test rgb_to_cmyk_pdf
    $testsFailed = $LASTEXITCODE -ne 0

    if ($hasRemainingRgb) {
        Write-Host 'Remaining RGB markers in decoded output:' -ForegroundColor Yellow
        $remainingRgb | ForEach-Object { Write-Host $_ }
    }

    if ($hasRemainingRgb -or $testsFailed) {
        throw 'RGB-to-CMYK acceptance gate is not yet satisfied.'
    }

    Write-Host 'RGB-to-CMYK acceptance gate passed.' -ForegroundColor Green
}
finally {
    Pop-Location
}
