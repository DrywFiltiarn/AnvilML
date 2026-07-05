#!/usr/bin/env bash
# install_worker_deps.sh — Create a Python venv and install dependencies for
# the AnvilML Python worker.
#
# Usage:
#   bash scripts/install_worker_deps.sh --mode=agent
#   bash scripts/install_worker_deps.sh --mode=ci
#
# --mode is required, no default:
#   agent  Installs the CPU torch build from worker/requirements/cpu-linux-agent.txt
#          (Forge agent / local developer environment — ANVILML_DESIGN.md §3.1)
#   ci     Installs the CPU torch build from worker/requirements/cpu-runner-reqs.txt
#          (GitHub CI runners — ANVILML_DESIGN.md §3.1)
#
# Environment variables:
#   ANVILML_VENV_PATH  Path to the venv root (default: ./worker/.venv)
#
# This script is idempotent: if the venv already exists, it skips creation and
# re-installs dependencies from worker/requirements/base.txt plus the torch
# wheel pin for the selected --mode.
#
# NOTE: this script does not detect or select a hardware backend (CUDA / ROCm).
# Both modes install a CPU-only torch build. GPU provisioning is manual today
# (see worker/requirements/rocm-linux.txt).

set -euo pipefail

# ── Parse arguments ─────────────────────────────────────────────────────────
mode=""
for arg in "$@"; do
    case "$arg" in
        --mode=*)
            mode="${arg#--mode=}"
            ;;
        *)
            echo "error: unrecognized argument: $arg" >&2
            echo "usage: bash scripts/install_worker_deps.sh --mode=agent|ci" >&2
            exit 1
            ;;
    esac
done

case "$mode" in
    agent)
        torch_reqs_file="worker/requirements/cpu-linux-agent.txt"
        ;;
    ci)
        torch_reqs_file="worker/requirements/cpu-runner-reqs.txt"
        ;;
    "")
        echo "error: --mode is required (agent|ci)" >&2
        echo "usage: bash scripts/install_worker_deps.sh --mode=agent|ci" >&2
        exit 1
        ;;
    *)
        echo "error: invalid --mode value: '$mode' (must be agent|ci)" >&2
        exit 1
        ;;
esac

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

# Install the CPU torch build for the selected mode.
echo "installing torch (mode=$mode) from $torch_reqs_file"
pip install -r "$torch_reqs_file"
