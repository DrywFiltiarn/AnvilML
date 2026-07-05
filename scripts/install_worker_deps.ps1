# install_worker_deps.ps1 -- Create a Python venv and install dependencies for
# the AnvilML Python worker.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\install_worker_deps.ps1 --mode=agent
#   powershell -ExecutionPolicy Bypass -File scripts\install_worker_deps.ps1 --mode=ci
#
# --mode is required, no default:
#   agent  Installs the CPU torch build from worker\requirements\cpu-linux-agent.txt
#          (Forge agent / local developer environment -- ANVILML_DESIGN.md §3.1)
#   ci     Installs the CPU torch build from worker\requirements\cpu-runner-reqs.txt
#          (GitHub CI runners -- ANVILML_DESIGN.md §3.1)
#
# Environment variables:
#   ANVILML_VENV_PATH  Path to the venv root (default: .\worker\.venv)
#
# This script is idempotent: if the venv already exists, it skips creation and
# re-installs dependencies from worker/requirements/base.txt plus the torch
# wheel pin for the selected --mode.
#
# NOTE: this script does not detect or select a hardware backend (CUDA / ROCm).
# Both modes install a CPU-only torch build. GPU provisioning is manual today
# (see worker/requirements/rocm-windows.txt).
#
# Requires: py -3.12 (installed by the standard Python 3.12 installer).
# Compatible with PowerShell 5.1 and later.

$ErrorActionPreference = 'Stop'

# ── Parse arguments ──────────────────────────────────────────────────────────
$mode = $null
foreach ($arg in $args) {
    if ($arg -like '--mode=*') {
        $mode = $arg.Substring(7)
    } else {
        Write-Error "error: unrecognized argument: $arg`nusage: install_worker_deps.ps1 --mode=agent|ci"
        exit 1
    }
}

if ($null -eq $mode -or $mode -eq '') {
    Write-Error "error: --mode is required (agent|ci)`nusage: install_worker_deps.ps1 --mode=agent|ci"
    exit 1
}

switch ($mode) {
    'agent' { $torchReqsFile = 'worker\requirements\cpu-linux-agent.txt' }
    'ci'    { $torchReqsFile = 'worker\requirements\cpu-runner-reqs.txt' }
    default {
        Write-Error "error: invalid --mode value: '$mode' (must be agent|ci)"
        exit 1
    }
}

# Verify that py -3.12 is available (installed by the standard Python 3.12 installer).
# This is a hard requirement -- no fallback to other Python versions.
# try/catch used because || is not valid PowerShell syntax.
try {
    py -3.12 -c "import sys" 2>$null
} catch {
    Write-Error "error: py -3.12 is required but not found on PATH"
    exit 1
}

# Resolve venv path from environment or use the documented default.
# Explicit null/empty check used instead of ?? for PowerShell 5.1 compatibility.
if ($null -ne $env:ANVILML_VENV_PATH -and $env:ANVILML_VENV_PATH -ne '') {
    $venv_path = $env:ANVILML_VENV_PATH
} else {
    $venv_path = '.\worker\.venv'
}

# If the venv python.exe already exists, skip creation (idempotency).
if (Test-Path (Join-Path $venv_path 'Scripts\python.exe')) {
    Write-Host "venv already exists at $venv_path -- skipping creation"
} else {
    Write-Host "creating venv at $venv_path"
    py -3.12 -m venv $venv_path
}

# Activate the venv by dot-sourcing the activation script.
& (Join-Path $venv_path 'Scripts\Activate.ps1')

# Install the base dependencies declared in requirements\base.txt.
pip install -r worker\requirements\base.txt

# Install the CPU torch build for the selected mode.
Write-Host "installing torch (mode=$mode) from $torchReqsFile"
pip install -r $torchReqsFile
