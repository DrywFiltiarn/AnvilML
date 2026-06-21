# Tasks: Phase 021 — Auto-Provisioning & Version Introspection

| Field | Value |
|-------|-------|
| Phase | 021 |
| Name | Auto-Provisioning & Version Introspection |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 20 |

## Overview

Phase 021 adds two operational quality-of-life features. First, `GET /v1/system/versions` exposes a machine-readable report of all component versions so monitoring tools and frontends can display runtime version information without parsing log output. Second, the server automatically provisions the Python worker virtual environment on first run, eliminating the manual provisioning step required in earlier phases.

**This phase also corrects a sequencing defect found during a later authoring review.** The original phase plan called `scripts/install_worker_deps.sh`/`.ps1` from the auto-provisioning task while still relying on a *separate, later* task (originally `P22-A2`, in Phase 022) to add GPU hardware detection to those same scripts. A fresh install going through Phase 021's auto-provisioning, before Phase 022 had run, would install CPU-only or undetected-backend torch — not because anything was left unimplemented with an expected later fix (the `defers_to`-shaped defect that motivated `FORGE_TASK_AUTHORING_SPEC.md §12a`), but because the *phase order itself* placed the consumer before the capability it needed. The fix: hardware detection is now `P21-A2`, sequenced before auto-provisioning (`P21-A3`) within this same phase, so auto-provisioning always calls hardware-aware scripts. The task IDs are numbered to match execution order: `P21-A1` → `P21-A2` → `P21-A3`.

At phase start the server requires a pre-provisioned venv before workers can start. At phase end, a freshly installed binary with no venv present will provision itself in the background, correctly detecting CUDA/ROCm/CPU and installing the matching torch build: the server binds immediately, `/health` returns 200, job submissions return 503 with `provisioning_in_progress` until the venv is ready, and then workers spawn automatically.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | anvilml-core + server + backend + scripts | P21-A1, P21-A2, P21-A3 | ComponentVersions type and endpoint; provisioning scripts extended with hardware detection (moved from Phase 022); auto-provisioning background task |

## Prerequisites

Phase 020 complete: all 6 CI jobs pass, `TESTS.md` catalogue complete, clippy clean. `ProvisioningState` enum (`NotStarted`, `Provisioning`, `Ready`, `Failed`) exists in `anvilml-core`. `GET /v1/system/env` returns `EnvReport` including `provisioning` state. `scripts/install_worker_deps.sh` and `.ps1` exist from Phase 008 (P8-B3) with base dependency installation only — `P21-A2` (below) extends them with hardware detection before `P21-A3`'s auto-provisioning task ever calls them, closing the gap described above.

## Task Descriptions

### Group A — anvilml-core and backend

#### P21-A1: ComponentVersions type and GET /v1/system/versions

**Goal:** Define `ComponentVersions` in `anvilml-core` and expose it via `GET /v1/system/versions`. All four version strings are populated at startup: the Rust binary version from `CARGO_PKG_VERSION`, the Python interpreter version, the `torch` version, and the IPC protocol version — all sourced from the `EnvReport` that the worker probe already populates.

**Files to create or modify:**
- `crates/anvilml-core/src/types/versions.rs` — new file; `ComponentVersions` struct
- `crates/anvilml-core/src/types/mod.rs` — re-export `ComponentVersions`
- `crates/anvilml-server/src/handlers/system.rs` — add `get_versions` handler
- `crates/anvilml-server/src/lib.rs` — mount `GET /v1/system/versions`

**Key implementation notes:**
- `ComponentVersions { anvilml: String, python: String, torch: String, worker_protocol: String }` — all fields are non-optional; use `"unknown"` if a field is not yet populated
- Handler reads from `AppState.env_report` (already an `Arc<RwLock<EnvReport>>`); extracts `python_version` and `torch_version`; uses `env!("CARGO_PKG_VERSION")` for `anvilml`; hardcodes the IPC protocol version string (e.g. `"1"` or a semver constant in `anvilml-core`)
- `curl http://127.0.0.1:8488/v1/system/versions` must return 200 with all four fields non-empty

**Acceptance criterion:** `cargo test --features mock-hardware` exits 0; integration test verifies `GET /v1/system/versions` returns 200 with non-empty values for all four fields.

---

#### P21-A2: Provisioning scripts extended with hardware detection and torch installation

**Goal:** Extend the existing `scripts/install_worker_deps.sh` and `scripts/install_worker_deps.ps1` (created in P8-B3) to detect the available GPU backend and install the matching torch build after the base dependencies. This completes the full provisioning flow referenced in `ANVILML_DESIGN.md §18.1`.

**Relocated from Phase 022 during a later authoring review.** This task was originally `P22-A2`. It is moved here, before `P21-A3` (auto-provisioning), because `P21-A3` calls these exact scripts — running hardware detection one phase later than the auto-provisioning that depends on it was a genuine sequencing defect (see this phase's Overview). Phase 022 is otherwise unaffected; it retains only its release-packaging task.

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

#### P21-A3: backend: auto-provisioning background task on first run

**Goal:** Implement first-run auto-provisioning in `backend/src/main.rs`. On startup, if the worker venv is absent or `import torch` fails, a background task runs the provisioning script — now hardware-aware, per `P21-A2` above — before spawning workers. The server must bind and serve `/health` immediately even while provisioning is in progress.

**Files to create or modify:**
- `backend/src/main.rs` — add venv check and background provisioning task before `WorkerPool::spawn_all`
- `crates/anvilml-server/src/handlers/system.rs` — ensure `GET /v1/system/env` reflects live `ProvisioningState`

**Key implementation notes:**
- Startup sequence: open DB → check venv → if venv absent or torch missing: set `ProvisioningState::Provisioning` in `EnvReport`; spawn `tokio::task::spawn_blocking` running `scripts/install_worker_deps.sh` (Linux) or `.ps1` (Windows via PowerShell), selected via `#[cfg(target_os)]`
- Emit `WsEvent::ProvisioningProgress` frames during provisioning (at minimum: Started and Finished)
- `WorkerPool::spawn_all` is deferred via a `tokio::sync::Notify` that fires when provisioning reaches `ProvisioningState::Ready`
- Job submissions return `AnvilError::WorkersUnavailable` (503) while provisioning; this is already the correct behaviour since no workers are Idle
- `cargo run --features mock-hardware` with no venv present: server binds and `/health` returns 200; `GET /v1/system/env` shows `{ provisioning: "Provisioning" }`

**Acceptance criterion:** `cargo test --workspace --features mock-hardware` exits 0; manual smoke test: delete `worker/.venv`; `cargo run --features mock-hardware`; `curl /health` returns 200 within 2 s; `curl /v1/system/env` shows `provisioning: "Provisioning"`.

---

## Phase Acceptance Criteria

```bash
cargo test --workspace --features mock-hardware
bash -n scripts/install_worker_deps.sh
bash scripts/install_worker_deps.sh && worker/.venv/bin/python3 -c "import torch"
# Runnable Proof (manual): version introspection endpoint on a live server
cargo run --features mock-hardware &
sleep 5
curl -s http://127.0.0.1:8488/v1/system/versions | python3 -c "import sys,json; d=json.load(sys.stdin); assert all(d[k] for k in ('anvilml','python','torch','worker_protocol'))"
# -> 200 with anvilml, python, torch, worker_protocol all non-empty
kill %1
```

## Known Constraints and Gotchas

- The server must bind and serve `/health` immediately even while provisioning runs in the background. Job submissions return 503 until provisioning completes. Do not defer `axum::serve` until after provisioning.
- On Windows, the provisioning script is `.ps1` and must be invoked via `powershell.exe -ExecutionPolicy Bypass -File scripts/install_worker_deps.ps1`. The invocation is platform-gated via `#[cfg(target_os = "windows")]`.
- The IPC protocol version string must be defined as a constant in `anvilml-core` (not hardcoded as a string literal in the handler) so it can be referenced from integration tests.
- `install_worker_deps.sh` and `.ps1` were created in P8-B3 with base dependencies only. `P21-A2` extends them with hardware detection; it does not recreate them. The agent must read the existing files before modifying them.
- Follow `FORGE_AGENT_RULES.md §12` for all inline documentation.
- Follow `FORGE_AGENT_RULES.md §11` for all logging.