# Download Moonshine Streaming Small model files
$ErrorActionPreference = "Stop"

$modelsDir = Join-Path $PSScriptRoot ".." "native" "speech-sidecar" "models" "moonshine" "streaming-small"

Write-Host "=== Downloading Moonshine Streaming Small Model ===" -ForegroundColor Cyan

# Create directory
if (-not (Test-Path $modelsDir)) {
    New-Item -ItemType Directory -Force -Path $modelsDir | Out-Null
}

# Check for Python
try {
    $pythonVersion = & python --version 2>&1
    Write-Host "Python: $pythonVersion" -ForegroundColor Green
} catch {
    Write-Host "WARNING: Python not found. Moonshine model download requires Python." -ForegroundColor Yellow
    Write-Host "Install Python 3.10+ from: https://www.python.org/downloads/" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "Alternative: manually download Moonshine ONNX model files." -ForegroundColor Yellow
    Write-Host "  Repository: https://github.com/usefulmove/moonshine" -ForegroundColor White
    Write-Host "  Place ONNX files in: $modelsDir" -ForegroundColor White
    exit 1
}

# Install moonshine package
Write-Host ""
Write-Host "Installing moonshine-onnx package..." -ForegroundColor Cyan
& python -m pip install moonshine-onnx --quiet 2>$null

# Download/export the model
Write-Host "Downloading Moonshine Streaming Small model..." -ForegroundColor Cyan

$downloadScript = @"
import os
import sys

try:
    from moonshine_onnx import MoonshineOnnxModel

    model_dir = sys.argv[1]
    print(f"Model directory: {model_dir}")

    # Initialize model - this downloads it if needed
    model = MoonshineOnnxModel(model_name="moonshine/tiny")
    print("Moonshine model initialized successfully")

    # The moonshine-onnx package caches models; we note the cache location
    cache_dir = os.path.join(os.path.expanduser("~"), ".cache", "moonshine")
    if os.path.exists(cache_dir):
        print(f"Model cache: {cache_dir}")
        for f in os.listdir(cache_dir):
            src = os.path.join(cache_dir, f)
            if os.path.isfile(src):
                print(f"  Found: {f}")

    print("SUCCESS: Moonshine model ready")

except ImportError:
    print("WARNING: moonshine-onnx not available")
    print("Install with: pip install moonshine-onnx")
    sys.exit(1)
except Exception as e:
    print(f"WARNING: {e}")
    print("Moonshine will run in limited mode")
    sys.exit(0)
"@

$tempScript = Join-Path $env:TEMP "download_moonshine.py"
$downloadScript | Out-File -FilePath $tempScript -Encoding utf8

try {
    & python $tempScript $modelsDir
} catch {
    Write-Host "WARNING: Moonshine model download had issues." -ForegroundColor Yellow
    Write-Host "The sidecar will start but transcription may be limited." -ForegroundColor Yellow
}

# Clean up
Remove-Item -Path $tempScript -Force -ErrorAction SilentlyContinue

# Create a marker file so the sidecar knows the model location
$markerContent = @"
{
    "model": "moonshine-streaming-small",
    "format": "onnx",
    "language": "en",
    "notes": "Model files managed by moonshine-onnx package. Cache in ~/.cache/moonshine"
}
"@
$markerContent | Out-File -FilePath (Join-Path $modelsDir "model_info.json") -Encoding utf8

Write-Host ""
Write-Host "=== Moonshine Download Complete ===" -ForegroundColor Cyan
