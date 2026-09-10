# Download Silero VAD ONNX model
$ErrorActionPreference = "Stop"

$modelsDir = Join-Path $PSScriptRoot ".." "native" "speech-sidecar" "models" "silero"
$modelFile = Join-Path $modelsDir "silero_vad.onnx"
$modelUrl = "https://github.com/snakers4/silero-vad/raw/master/src/silero_vad/data/silero_vad.onnx"

Write-Host "=== Downloading Silero VAD Model ===" -ForegroundColor Cyan

# Create directory
if (-not (Test-Path $modelsDir)) {
    New-Item -ItemType Directory -Force -Path $modelsDir | Out-Null
}

# Check if already downloaded
if (Test-Path $modelFile) {
    $size = (Get-Item $modelFile).Length / 1MB
    Write-Host "Silero VAD model already exists ($([math]::Round($size, 2)) MB)" -ForegroundColor Yellow
    Write-Host "Delete $modelFile to re-download." -ForegroundColor Gray
    exit 0
}

# Download
Write-Host "Downloading from: $modelUrl" -ForegroundColor White
try {
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    Invoke-WebRequest -Uri $modelUrl -OutFile $modelFile -UseBasicParsing

    $size = (Get-Item $modelFile).Length / 1MB
    Write-Host "Downloaded successfully: $([math]::Round($size, 2)) MB" -ForegroundColor Green
} catch {
    Write-Host "ERROR: Failed to download Silero VAD model." -ForegroundColor Red
    Write-Host $_.Exception.Message -ForegroundColor Red
    Write-Host ""
    Write-Host "Manual download:" -ForegroundColor Yellow
    Write-Host "  1. Go to: https://github.com/snakers4/silero-vad/tree/master/src/silero_vad/data" -ForegroundColor White
    Write-Host "  2. Download silero_vad.onnx" -ForegroundColor White
    Write-Host "  3. Place it in: $modelsDir" -ForegroundColor White
    exit 1
}

Write-Host "=== Silero VAD Download Complete ===" -ForegroundColor Cyan
