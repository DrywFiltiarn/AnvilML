# seed_tokenizers.ps1 -- Re-seed the Qwen3 tokenizer directory from the canonical
# HuggingFace upstream source.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File worker\tools\seed_tokenizers.ps1
#   powershell -ExecutionPolicy Bypass -File worker\tools\seed_tokenizers.ps1 -DryRun
#   powershell -ExecutionPolicy Bypass -File worker\tools\seed_tokenizers.ps1 -Source "REPO"
#
# Parameters:
#   -DryRun      Print what would be downloaded without making network calls.
#   -Source      HuggingFace repo to download from (default: Qwen/Qwen3-4B).
#
# Environment variables:
#   HF_TOKEN     HuggingFace authentication token (optional -- unauthenticated
#                requests work but are rate-limited; set this for higher rate
#                limits and faster downloads).
#
# Provenance -- why Qwen/Qwen3-4B is the canonical source:
#   - The tokenizer vocabulary is shared across ALL Qwen3 variants (4B, 8B, 32B,
#     235B). They all use the identical tokenizer files.
#   - Qwen/Qwen3-4B is the smallest Qwen3 release and the first official release
#     from the Qwen team at Alibaba Group.
#   - This is the authoritative upstream; the tokeniser is not a third-party
#     fork or community conversion.
#
# This script is idempotent: if files already exist in the output directory,
# they are overwritten to guarantee freshness.
#
# Requires: py -3.12 (installed by the standard Python 3.12 installer) and
#           the `hf` or `huggingface-cli` command on PATH.

$ErrorActionPreference = 'Stop'

# ── Defaults ──────────────────────────────────────────────────────────────────
param(
    [switch]$DryRun,
    [string]$Source = 'Qwen/Qwen3-4B'
)

# Tokenizer files to download from the upstream repo.
# These are the exact files present in the Qwen/Qwen3-4B repo that constitute
# the complete tokenizer -- no special_tokens_map.json or added_tokens.json
# exist in this repo, so they are not requested.
$TokenizerFiles = @(
    'tokenizer.json'
    'tokenizer_config.json'
    'vocab.json'
    'merges.txt'
)

$OutputDir = 'worker\assets\qwen3_tokenizer'

# ── Dry-run path ──────────────────────────────────────────────────────────────
if ($DryRun) {
    Write-Host "dry-run: would download the following tokenizer files from $Source"
    Write-Host "         into $OutputDir"
    Write-Host ""
    foreach ($f in $TokenizerFiles) {
        Write-Host "  - $f"
    }
    Write-Host ""
    Write-Host "Provenance: Qwen/Qwen3-4B is the canonical Qwen3 tokenizer source."
    Write-Host "  Tokenizer vocabulary is shared across all Qwen3 variants (4B, 8B, 32B, 235B)."
    Write-Host "  This is the official release from the Qwen team at Alibaba Group."
    exit 0
}

# ── Resolve CLI tool ─────────────────────────────────────────────────────────
# Prefer `hf` (the new CLI from huggingface_hub >= 0.23) over the deprecated
# `huggingface-cli`. Both are Python packages available on any platform.
$HfCli = $null
try {
    hf --version 2>$null
    if ($LASTEXITCODE -eq 0) { $HfCli = 'hf' }
} catch {}

if (-not $HfCli) {
    try {
        huggingface-cli --version 2>$null
        if ($LASTEXITCODE -eq 0) { $HfCli = 'huggingface-cli' }
    } catch {}
}

if (-not $HfCli) {
    Write-Error "error: neither 'hf' nor 'huggingface-cli' found on PATH"
    Write-Error "install with: pip install huggingface_hub"
    exit 1
}

# ── Download ──────────────────────────────────────────────────────────────────
Write-Host "downloading tokenizer files from $Source into $OutputDir"

# Create output directory if it doesn't exist.
if (-not (Test-Path $OutputDir)) {
    New-Item -ItemType Directory -Path $OutputDir | Out-Null
}

# Build the positional file arguments for the CLI.
# The `hf` CLI takes filenames as positional args after --local-dir.
$FileArgs = $TokenizerFiles -join ' '

& $HfCli download $Source $FileArgs --local-dir $OutputDir

$FileCount = (Get-ChildItem $OutputDir -File | Measure-Object).Count
Write-Host "done: $FileCount file(s) in $OutputDir"
