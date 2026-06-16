# Tasks: Phase 022 — Release Packaging

| Field | Value |
|-------|-------|
| Phase | 022 |
| Name | Release Packaging |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 21 |

## Overview

Phase 022 prepares the release distribution: a GitHub Actions workflow that builds and publishes signed release artifacts on tag push, and fully validated provisioning scripts for both platforms.

The provisioning scripts (`scripts/install_worker_deps.sh` and `.ps1`) were created in Phase 008 with base dependency installation only. Phase 022 extends them with hardware detection (CUDA, ROCm, CPU fallback) and torch installation, completing the full provisioning flow described in `ANVILML_DESIGN.md §18.1` and `ENVIRONMENT.md §1`.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | release | P22-A1 … P22-A2 | Release workflow, provisioning scripts extended with hardware detection |

## Prerequisites

Phase 021 complete. `scripts/install_worker_deps.sh` and `scripts/install_worker_deps.ps1` exist from Phase 008 (P8-B3) and install base dependencies into the worker venv. `worker/requirements/cuda.txt`, `rocm-linux.txt`, `rocm-windows.txt`, and `cpu.txt` must exist in `worker/requirements/`.

## Task Descriptions

### Group A

#### P22-A1: GitHub Release workflow

See context field.

#### P22-A2: Provisioning scripts extended with hardware detection and torch installation

**Goal:** Extend the existing `scripts/install_worker_deps.sh` and `scripts/install_worker_deps.ps1` (created in P8-B3) to detect the available GPU backend and install the matching torch build after the base dependencies. This completes the full provisioning flow referenced in `ANVILML_DESIGN.md §18.1`.

**Files to modify:**
- `scripts/install_worker_deps.sh` — append hardware detection block: `nvidia-smi` present → install `cuda.txt`; `amdgpu` or `rocminfo` present → install `rocm-linux.txt`; else install `cpu.txt`.
- `scripts/install_worker_deps.ps1` — append hardware detection block: `Get-CimInstance` or `amd-smi` for AMD → install `rocm-windows.txt`; `nvidia-smi` → install `cuda.txt`; else install `cpu.txt`.

**Key implementation notes:**
- Both scripts must remain idempotent. If the venv already contains torch, the pip install step should be a no-op (pip handles this).
- The `.sh` script already uses `set -euo pipefail` from P8-B3; do not remove it.
- The `.ps1` script already uses `$ErrorActionPreference = 'Stop'` from P8-B3; do not remove it.

**Acceptance criterion:** `bash -n scripts/install_worker_deps.sh` exits 0 (syntax check); PSScriptAnalyzer passes for `.ps1`; `bash scripts/install_worker_deps.sh && worker/.venv/bin/python3 -c "import torch"` exits 0.

## Phase Acceptance Criteria

```bash
cargo build --release
bash -n scripts/install_worker_deps.sh
bash scripts/install_worker_deps.sh
worker/.venv/bin/python3 -c "import torch"
```

## Known Constraints and Gotchas

- The release zip must contain `worker/` source so auto-provisioning works on first run. Python source is not embedded in the binary.
- `install_worker_deps.sh` and `.ps1` were created in P8-B3. P22-A2 extends them; it does not recreate them. The agent must read the existing files before modifying them.
- PSScriptAnalyzer must be installed on the Windows runner for the `.ps1` lint gate to work. Add an installation step to the release workflow if it is not available by default.