# install_worker_deps.ps1 — Create a Python venv and install base dependencies for
# the AnvilML Python worker.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\install_worker_deps.ps1
#
# Environment variables:
#   ANVILML_VENV_PATH  Path to the venv root (default: .\worker\.venv)
#
# This script is idempotent: if the venv already exists, it skips creation and
# re-installs dependencies from worker/requirements/base.txt.

$ErrorActionPreference = 'Stop'

# Verify that py -3.12 is available (installed by the standard Python 3.12 installer).
# This is a hard requirement — no fallback to other Python versions.
py -3.12 -c "import sys" 2>$null \
  || { Write-Error "error: py -3.12 is required but not found on PATH"; exit 1; }

# Resolve venv path from environment or use the documented default.
$venv_path = $env:ANVILML_VENV_PATH ?? ".\worker\.venv"

# If the venv's python.exe already exists, skip creation (idempotency).
if (Test-Path "$venv_path\Scripts\python.exe") {
    Write-Host "venv already exists at $venv_path — skipping creation"
} else {
    Write-Host "creating venv at $venv_path"
    py -3.12 -m venv "$venv_path"
}

# Activate the venv by dot-sourcing the activation script.
& "$venv_path\Scripts\Activate.ps1"

# Install the base dependencies declared in requirements\base.txt.
pip install -r worker\requirements\base.txt
