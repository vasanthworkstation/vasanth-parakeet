# Verify that all required models are present
$ErrorActionPreference = "Continue"

$modelsBase = Join-Path $PSScriptRoot ".." "native" "speech-sidecar" "models"
$allGood = $true

Write-Host "=== Verifying Speech Models ===" -ForegroundColor Cyan
Write-Host ""

# Check Silero VAD
$sileroPath = Join-Path $modelsBase "silero" "silero_vad.onnx"
if (Test-Path $sileroPath) {
    $size = (Get-Item $sileroPath).Length / 1MB
    Write-Host "[OK] Silero VAD: $([math]::Round($size, 2)) MB" -ForegroundColor Green
} else {
    Write-Host "[MISSING] Silero VAD model" -ForegroundColor Red
    Write-Host "  Run: powershell -ExecutionPolicy Bypass -File scripts/download-silero.ps1" -ForegroundColor Yellow
    $allGood = $false
}

# Check Moonshine
$moonshinePath = Join-Path $modelsBase "moonshine" "streaming-small"
$moonshineMarker = Join-Path $moonshinePath "model_info.json"
if (Test-Path $moonshineMarker) {
    Write-Host "[OK] Moonshine Streaming Small: model configured" -ForegroundColor Green
} else {
    Write-Host "[MISSING] Moonshine model" -ForegroundColor Red
    Write-Host "  Run: powershell -ExecutionPolicy Bypass -File scripts/download-moonshine.ps1" -ForegroundColor Yellow
    $allGood = $false
}

# Check sidecar executable
$sidecarRelease = Join-Path $PSScriptRoot ".." "native" "speech-sidecar" "target" "release" "speech-sidecar.exe"
$sidecarDebug = Join-Path $PSScriptRoot ".." "native" "speech-sidecar" "target" "debug" "speech-sidecar.exe"

if (Test-Path $sidecarRelease) {
    $size = (Get-Item $sidecarRelease).Length / 1MB
    Write-Host "[OK] Speech sidecar (release): $([math]::Round($size, 2)) MB" -ForegroundColor Green
} elseif (Test-Path $sidecarDebug) {
    $size = (Get-Item $sidecarDebug).Length / 1MB
    Write-Host "[OK] Speech sidecar (debug): $([math]::Round($size, 2)) MB" -ForegroundColor Yellow
} else {
    Write-Host "[MISSING] Speech sidecar executable" -ForegroundColor Red
    Write-Host "  Run: powershell -ExecutionPolicy Bypass -File scripts/build-speech-sidecar.ps1" -ForegroundColor Yellow
    $allGood = $false
}

# Check ONNX Runtime
Write-Host ""
Write-Host "--- ONNX Runtime ---" -ForegroundColor Gray
$onnxPaths = @(
    (Join-Path $modelsBase ".." "target" "release" "onnxruntime.dll"),
    "C:\Program Files\onnxruntime\lib\onnxruntime.dll"
)
$onnxFound = $false
foreach ($p in $onnxPaths) {
    if (Test-Path $p) {
        Write-Host "[OK] ONNX Runtime found: $p" -ForegroundColor Green
        $onnxFound = $true
        break
    }
}
if (-not $onnxFound) {
    Write-Host "[INFO] ONNX Runtime DLL not found in standard paths" -ForegroundColor Yellow
    Write-Host "  The ort crate may download it automatically during build." -ForegroundColor Gray
    Write-Host "  If VAD fails, install ONNX Runtime: https://github.com/microsoft/onnxruntime/releases" -ForegroundColor Gray
}

Write-Host ""
if ($allGood) {
    Write-Host "All components verified!" -ForegroundColor Green
} else {
    Write-Host "Some components are missing. See instructions above." -ForegroundColor Yellow
}

Write-Host "=== Verification Complete ===" -ForegroundColor Cyan
