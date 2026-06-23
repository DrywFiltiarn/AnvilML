<#
run_all.ps1 - run every real-path verification script in order.

Each script is also independently runnable; this just chains them so you
get one combined report instead of running five commands by hand. Unlike
`$ErrorActionPreference = 'Stop'`, this deliberately continues past a failing
script (e.g. LoadClip failing on the tokenizer path bug should not prevent
you from also seeing whether LoadModel and LoadVae are fine) and reports
an overall pass/fail count at the end.

Usage:
    $env:ANVILML_MODELS_DIR="C:\path\to\models"
    $env:ANVILML_ZIT_MODEL="zit_fp8.safetensors"
    $env:ANVILML_ZIT_VAE="zit_vae.safetensors"
    $env:ANVILML_ZIT_CLIP="qwen3_4b.safetensors"
    $env:ANVILML_DEVICE="cuda:0"       # optional, defaults to cuda:0
    .\run_all.ps1
#>

# Enforce strict variable declaration (similar to set -u)
Set-StrictMode -Version Latest

# Change directory to the location of this script (equivalent to cd "$(dirname "$0")")
Set-Location -Path $PSScriptRoot

if ($env:ANVILML_WORKER_MOCK -eq "1") {
    Write-Error "FATAL: ANVILML_WORKER_MOCK=1 is set - unset it before running this harness."
    exit 2
}

$env:ANVILML_MODELS_DIR="E:/AnvilML/models"
$env:ANVILML_ZIT_MODEL="z_image_turbo_fp8_e4m3fn.safetensors"
$env:ANVILML_ZIT_VAE="zit.vae.safetensors"
$env:ANVILML_ZIT_CLIP="qwen_3_4b_abliterated_v2.safetensors"
$env:ANVILML_DEVICE="cuda:0"

$Scripts = @(
    "01_loaders.py",
    "02_clip_encode.py",
    "03_empty_latent.py",
    "04_sampler.py",
    "05_vae_decode.py"
)

$Results = @()

foreach ($script in $Scripts) {
    Write-Host ""
    Write-Host "########################################################################"
    Write-Host "# $script"
    Write-Host "########################################################################"
    
    # Execute the python script. 
    # Note: 'python' is standard on Windows. If your environment specifically uses 'python3', adjust accordingly.
    & python $script
    
    # Capture the integer exit code of the last native command
    $rc = $LASTEXITCODE
    
    if ($rc -eq 0) {
        $Results += "PASS  $script"
    } else {
        $Results += "FAIL  $script  (exit $rc)"
    }
}

Write-Host ""
Write-Host "========================================================================"
Write-Host "OVERALL"
Write-Host "========================================================================"

$overall_rc = 0
foreach ($r in $Results) {
    Write-Host "  $r"
    if ($r -match "^FAIL") {
        $overall_rc = 1
    }
}
Write-Host "========================================================================"

$env:ANVILML_MODELS_DIR=""
$env:ANVILML_ZIT_MODEL=""
$env:ANVILML_ZIT_VAE=""
$env:ANVILML_ZIT_CLIP=""
$env:ANVILML_DEVICE=""

exit $overall_rc