#!/usr/bin/env bash
# install_worker_deps.sh — Create a Python venv and install base dependencies for
# the AnvilML Python worker.
#
# Usage:
#   bash scripts/install_worker_deps.sh
#
# Environment variables:
#   ANVILML_VENV_PATH  Path to the venv root (default: ./worker/.venv)
#
# This script is idempotent: if the venv already exists, it skips creation and
# re-installs dependencies from worker/requirements/base.txt.

set -euo pipefail

# Verify that python3.12 is available on PATH.
# This is a hard requirement — no fallback to other Python versions.
command -v python3.12 >/dev/null 2>&1 \
  || { echo "error: python3.12 is required but not found on PATH" >&2; exit 1; }

# Resolve venv path from environment or use the documented default.
venv_path="${ANVILML_VENV_PATH:-./worker/.venv}"

# If the venv's python3 already exists, skip creation (idempotency).
if [ -f "$venv_path/bin/python3" ]; then
    echo "venv already exists at $venv_path — skipping creation"
else
    echo "creating venv at $venv_path"
    python3.12 -m venv "$venv_path"
fi

# Activate the venv so that pip resolves to the venv's copy.
source "$venv_path/bin/activate"

# Install the base dependencies declared in requirements/base.txt.
pip install -r worker/requirements/base.txt
