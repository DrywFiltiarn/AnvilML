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

The provisioning scripts (`scripts/install_worker_deps.sh` and `.ps1`) were created in Phase 008 (P8-B3) with base dependency installation only. Phase 022 extends them with hardware detection (CUDA, ROCm, CPU fallback) and torch installation, completing the full provisioning flow described in `ANVILML_DESIGN.md §18.1` and `ENVIRONMENT.md §1`.

At phase end, pushing a `v*.*.*` tag triggers a CI workflow that produces a release zip for Linux and Windows. Each zip contains the binary, the `worker/` source tree, `anvilml.toml`, and database seeds. `SHA256SUMS` is generated for verification.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | release | P22-A1 … P22-A2 | GitHub Release workflow; provisioning scripts extended with hardware detection |

## Prerequisites

Phase 021 complete. `cargo build --release` exits 0. `scripts/install_worker_deps.sh` and `scripts/install_worker_deps.ps1` exist from Phase 008 (P8-B3) and install base dependencies into the worker venv. `worker/requirements/cuda.txt`, `rocm-linux.txt`, `rocm-windows.txt`, and `cpu.txt` exist.

## Task Descriptions

### Group A — release

#### P22-A1: backend: cargo build --release and GitHub Release workflow

**Goal:** Create `.github/workflows/release.yml` triggered on `v*.*.*` tag push. The workflow builds release binaries for Linux and Windows, packages them into distributable archives, generates `SHA256SUMS`, and publishes a GitHub Release with all artifacts attached.

**Files to create or modify:**
- `.github/workflows/release.yml` — new file; two build jobs and one release job

**Key implementation notes:**
- `build-linux` job: `ubuntu-latest`; `cargo build --release`; create `anvilml-linux-x64.tar.gz` containing the binary + `worker/` + `anvilml.toml` + `database/seeds/`
- `build-windows` job: `windows-latest`; `cargo build --release --target x86_64-pc-windows-msvc`; create `anvilml-windows-x64.zip` with same contents
- Release job: `sha256sum` over both archives → `SHA256SUMS` file; `gh release create ${{ github.ref_name }}` uploading all three files and a `CHANGELOG.md` excerpt as the release body
- `cargo build --release` must exit 0 locally before this task is considered complete

**Acceptance criterion:** `cargo build --release` exits 0; release zip (manually constructed from the workflow's artifact packaging logic) contains the `anvilml` binary, `worker/` directory, and `anvilml.toml`; `sha256sum --check SHA256SUMS` exits 0 against the archive.

---

#### P22-A2: Provisioning scripts extended with hardware detection and torch installation

**Goal:** Extend the existing `scripts/install_worker_deps.sh` and `scripts/install_worker_deps.ps1` (created in P8-B3) to detect the available GPU backend and install the matching torch build after the base dependencies. This completes the full provisioning flow referenced in `ANVILML_DESIGN.md §18.1`.

**Files to create or modify:**
- `scripts/install_worker_deps.sh` — append hardware detection block
- `scripts/install_worker_deps.ps1` — append hardware detection block

**Key implementation notes:**
- `.sh` hardware detection: `nvidia-smi` present → `pip install -r worker/requirements/cuda.txt`; `amdgpu` module loaded or `rocminfo` present → `pip install -r worker/requirements/rocm-linux.txt`; else → `pip install -r worker/requirements/cpu.txt`
- `.ps1` hardware detection: `Get-CimInstance Win32_VideoController` filtered for AMD vendor or `amd-smi` present → `rocm-windows.txt`; `nvidia-smi` → `cuda.txt`; else → `cpu.txt`
- Both scripts must remain idempotent: if `torch` is already importable, pip installs are no-ops
- The `.sh` script already uses `set -euo pipefail` from P8-B3; do not remove it
- The `.ps1` script already uses `$ErrorActionPreference = 'Stop'` from P8-B3; do not remove it
- **Read the existing files before modifying** — do not recreate them from scratch

**Acceptance criterion:** `bash -n scripts/install_worker_deps.sh` exits 0 (syntax check); PSScriptAnalyzer passes for `.ps1`; `bash scripts/install_worker_deps.sh && worker/.venv/bin/python3 -c "import torch"` exits 0.

---

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
