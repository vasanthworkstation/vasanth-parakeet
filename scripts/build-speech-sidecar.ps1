# Build the native speech sidecar
# Requires: Rust toolchain (rustup, cargo)

$ErrorActionPreference = "Stop"
$sidecarDir = Join-Path $PSScriptRoot ".." "native" "speech-sidecar"

Write-Host "=== Building Speech Sidecar ===" -ForegroundColor Cyan

# Check for Rust
try {
    $rustVersion = & rustc --version 2>&1
    Write-Host "Rust: $rustVersion" -ForegroundColor Green
} catch {
    Write-Host "ERROR: Rust is not installed." -ForegroundColor Red
    Write-Host "Install from: https://rustup.rs/" -ForegroundColor Yellow
    Write-Host "After installing, restart your terminal and run this script again." -ForegroundColor Yellow
    exit 1
}

# Check for cargo
try {
    $cargoVersion = & cargo --version 2>&1
    Write-Host "Cargo: $cargoVersion" -ForegroundColor Green
} catch {
    Write-Host "ERROR: Cargo not found." -ForegroundColor Red
    exit 1
}

# Build in release mode
Write-Host ""
Write-Host "Building speech-sidecar in release mode..." -ForegroundColor Cyan
Push-Location $sidecarDir

try {
    & cargo build --release
    if ($LASTEXITCODE -ne 0) {
        Write-Host "ERROR: Build failed!" -ForegroundColor Red
        Pop-Location
        exit 1
    }
} finally {
    Pop-Location
}

$exePath = Join-Path $sidecarDir "target" "release" "speech-sidecar.exe"
if (Test-Path $exePath) {
    $size = (Get-Item $exePath).Length / 1MB
    Write-Host ""
    Write-Host "Build successful!" -ForegroundColor Green
    Write-Host "  Executable: $exePath" -ForegroundColor White
    Write-Host "  Size: $([math]::Round($size, 2)) MB" -ForegroundColor White
} else {
    Write-Host "ERROR: Executable not found after build." -ForegroundColor Red
    exit 1
}

# Verify models exist
Write-Host ""
Write-Host "Checking models..." -ForegroundColor Cyan
& powershell -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "verify-models.ps1")

Write-Host ""
Write-Host "=== Build Complete ===" -ForegroundColor Cyan
