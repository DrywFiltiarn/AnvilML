# Tasks: Phase 022 — Real Python Worker — SDXL & Hardening

| Field | Value |
|-------|-------|
| Phase | 022 |
| Name | Real Python Worker — SDXL & Hardening |
| Milestone group | Real inference |
| Depends on phases | 1-21 |
| Task file | `forge/tasks/tasks_phase022.json` |
| Tasks | 6 |

## Overview

Phase 22 completes the MVP: real SDXL nodes, the real provisioning scripts (`.sh`/`.ps1`), the standalone `test_inference.py` debug harness, the crash-recovery and full REST integration test suites, and the final green CI across Linux and Windows. After this phase both pipelines run on real hardware, the system survives worker crashes, and the entire automated test matrix is green.

Every task in this phase implements **one module or one endpoint** plus its test. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the **Runnable Proof** below passes.

## Tasks

| Task | Module / File | Summary |
|------|-------------|---------|
| P22-A1 | `worker/nodes/sdxl.py` | worker: nodes/sdxl.py real SDXL nodes (optional negative_prompt) |
| P22-A2 | `backend/scripts/install_worker_deps.sh + .ps1` | backend/scripts: install_worker_deps.sh + .ps1 (real provisioning) |
| P22-A3 | `backend/scripts/test_inference.py` | backend/scripts/test_inference.py standalone debug harness |
| P22-A4 | `backend/tests/api_crash.rs` | anvilml: crash-recovery integration test (kill worker mid-job) |
| P22-A5 | `backend/tests/api_health.rs` | anvilml: full REST integration test suite (health/jobs/models/workers/artifacts) |
| P22-A6 | `docs/PROOF_phase022.md` | anvilml: final CI green + SDXL/ZiT manual smoke documentation |

## Task details

#### P22-A1: worker: nodes/sdxl.py real SDXL nodes (optional negative_prompt)

- **Prereqs:** P21-A7
- **Tags:** reasoning

Create worker/nodes/sdxl.py: SdxlLoadPipeline (dual encoders), SdxlTextEncode (negative_prompt optional, default ''), SdxlSampler (steps 20, guidance 7.5, seed resolve, cancel callback), SdxlDecode. Slots exactly per 14.6. Mock branches mirror zit (sentinels/black image). Update nodes/__init__.py to import sdxl. pytest worker/tests/test_nodes_sdxl.py (mock) exits 0: output slots; missing negative_prompt defaults to ''.

#### P22-A2: backend/scripts: install_worker_deps.sh + .ps1 (real provisioning)

- **Prereqs:** P22-A1
- **Tags:** —

Create install_worker_deps.sh (Linux/macOS): detect nvidia-smi->cuda, amd-smi/rocminfo->rocm, else cpu; venv via uv venv --python 3.12 else python3.12 -m venv; pip install base.txt then {cuda|rocm|cpu}.txt; print torch version+device. Create install_worker_deps.ps1 (Windows): py -3.12 or uv; detect nvidia-smi->cuda, amd-smi or AMD PyTorch-on-Windows driver->rocm (ROCm IS supported on Windows, design 21.4), else cpu; Windows+rocm installs rocm-windows.txt (AMD PyTorch-on-Windows, ROCm>=7.2), NOT the Linux index. Both exit non-zero if Python 3.12 missing. Verify: bash -n / PSScriptAnalyzer pass.

#### P22-A3: backend/scripts/test_inference.py standalone debug harness

- **Prereqs:** P22-A2
- **Tags:** —

Create backend/scripts/test_inference.py per 20.5: argparse --model-type zit|sdxl --model-path --prompt --output --steps --seed --device. Adds worker/ to sys.path, builds the node graph directly (no IPC/server), runs via run_graph with a no-op emit that saves the PNG on ImageReady, prints elapsed + VRAM before/after. Exit 0 success / 1 on error with traceback. Verify: runs with a real model + provisioned venv producing an output PNG.

#### P22-A4: anvilml: crash-recovery integration test (kill worker mid-job)

- **Prereqs:** P22-A3
- **Tags:** reasoning

Create backend/tests/api_crash.rs (feature test-helpers): submit a mock job with ANVILML_MOCK_NODE_DELAY_MS so worker is Busy; get PID via pid_for; kill it (unix libc::kill, windows TerminateProcess, cfg-gated); assert WS worker.status Dead then job.failed{error:worker_crashed} then worker Respawning then Idle within timeout; submit a second job and assert it Completes. cargo test --features mock-hardware,test-helpers --test api_crash exits 0.

#### P22-A5: anvilml: full REST integration test suite (health/jobs/models/workers/artifacts)

- **Prereqs:** P22-A4
- **Tags:** reasoning

Create/consolidate backend/tests/api_health.rs, api_jobs.rs, api_models.rs, api_workers.rs, api_artifacts.rs covering each endpoint's success + error codes (404/409/422) using in-process app + mock worker + in-memory DB. These complement the WS/cancel/delete/crash tests already added. cargo test --workspace --features mock-hardware exits 0 on both Linux and Windows CI.

#### P22-A6: anvilml: final CI green + SDXL/ZiT manual smoke documentation

- **Prereqs:** P22-A5
- **Tags:** —

Confirm CI fully green: rust-linux (fmt+clippy+test+openapi-diff), rust-windows (clippy+test), python-worker (pytest). Regenerate+commit backend/openapi.json. Write docs/PROOF_phase022.md documenting the four manual smoke tests (phase-1 startup, ZiT e2e, SDXL e2e, crash recovery) per ANVILML_DESIGN 20.4. Complete when the last main-branch CI run shows all jobs green and a human has run both ZiT and SDXL end-to-end producing real images.


## Runnable Proof

Generate with both pipelines on real hardware and confirm the full CI matrix is green.

```bash
# real hardware:
cp <your-sdxl-model>.safetensors models/diffusion/
ANVILML_VENV_PATH=./venv cargo run --release
# submit an SDXL job; confirm a real image via /v1/artifacts/<hash>
# crash recovery: kill a worker mid-job; confirm the job fails, worker respawns, next job succeeds
# debug harness:
python backend/scripts/test_inference.py --model-type sdxl --model-path <m> --prompt "a fox" --output fox.png

# CI (hermetic) — all green:
cargo test --workspace --features mock-hardware     # Linux AND Windows
ANVILML_WORKER_MOCK=1 pytest worker/tests
cargo run -p anvilml-openapi && git diff --exit-code backend/openapi.json
```

Expected: SDXL produces a real image; killing a worker mid-job is recovered automatically; `test_inference.py` writes a real PNG; and all CI jobs (rust-linux, rust-windows, python-worker, openapi-diff) are green. Phase done when both pipelines work end-to-end, crash recovery passes, and the full CI matrix is green on the main branch.
