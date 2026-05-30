# Tasks: Phase 010 — SDXL & Hardening

| Field            | Value                                                                       |
|------------------|-----------------------------------------------------------------------------|
| Phase            | 010                                                                         |
| Name             | SDXL & Hardening                                                            |
| ANVIL Milestone  | M6                                                                          |
| Status           | Draft                                                                       |
| Depends on phases| 1, 2, 3, 4, 5, 6, 7, 8, 9                                                   |
| Task file        | `forge/tasks/tasks_phase010.json`                                           |
| Design reference | `ANVILML_DESIGN.md` §9.4 (Cancellation), §7.5 (Watchdog), §14.6 (SDXL nodes), §20 (Testing), §21.4 (Scripts) |

---

## Overview

Phase 010 completes M6 — the final milestone of the AnvilML MVP. It adds the SDXL node set, hardens the system against the two most important failure modes (worker crash mid-job and cooperative job cancellation), finalises the CI pipeline, and delivers the real provisioning scripts and the standalone inference debug harness.

M6 exit criterion: "Both pipelines run; cancel + crash-recovery smoke pass; CI fully green." The two pipeline smoke tests are manual (§20.4); everything else is automated. When this phase is complete, the entire automated CI pipeline is green on both Linux and Windows, `backend/openapi.json` is stable and passing its diff gate, and the repo is in a state where a developer can follow the operations runbook (§22) to run the backend on real hardware.

The three hardening tasks (P10-A1 to P10-A3) are sequenced by dependency: SDXL nodes must exist before the cancel and crash tests can exercise them (cancel and crash recovery are more interesting with a multi-node pipeline). P10-B1 through P10-B3 are the CI, provisioning, and debug harness tasks that close out the repository.

---

## Group Reference

| Group | Subsystem | Tasks           | Summary                                                      |
|-------|-----------|-----------------|--------------------------------------------------------------|
| A     | hardening | P10-A1 … P10-A3 | SDXL nodes, cancel path end-to-end, crash recovery           |
| B     | delivery  | P10-B1 … P10-B3 | CI green gate, provisioning scripts, debug harness           |

---

## Prerequisites

- P9-B2 complete: full Python worker with ZiT nodes, mock mode, and parity test all passing.
- All Rust and Python tests green on the current state of the repo.

---

## Contract Documents Applicable to This Phase

| Document section           | Relevant tasks | What must match                                                      |
|----------------------------|----------------|----------------------------------------------------------------------|
| `ANVILML_DESIGN.md` §14.6  | P10-A1         | SDXL node input/output slot names; negative_prompt optional          |
| `ANVILML_DESIGN.md` §9.4   | P10-A2         | Full cancel flow: CancelJob IPC → Cancelled event → DB status; 409 on terminal |
| `ANVILML_DESIGN.md` §7.5   | P10-A3         | Watchdog path: EOF → Dead → Failed("worker_crashed") → Respawning → Idle |
| `ANVILML_DESIGN.md` §20.4  | P10-A2, P10-A3 | Smoke test descriptions (manual verification)                        |
| `ANVILML_DESIGN.md` §21.4  | P10-B2         | Script logic: detect backend, create venv, install requirements      |
| `ANVILML_DESIGN.md` §20.5  | P10-B3         | Debug harness: CLI flags, direct pipeline execution, timing output   |

---

## Task Descriptions

### Group A — Hardening

#### P10-A1: worker/nodes/sdxl.py — SDXL node set

**Goal:** Implement all four SDXL inference nodes, mirroring the ZiT implementation pattern and supporting `negative_prompt` as an optional input.

**Files to create or modify:**
- `worker/nodes/sdxl.py` — `SdxlLoadPipeline`, `SdxlTextEncode`, `SdxlSampler`, `SdxlDecode`

**Key implementation notes:**
- Declare `INPUT_SLOTS` and `OUTPUT_SLOTS` exactly per `ANVILML_DESIGN.md §14.6`.
- `SdxlTextEncode.INPUT_SLOTS = ['pipeline', 'prompt', 'negative_prompt']`. The `negative_prompt` input is optional; if not supplied in the graph, default to an empty string `""`. Access it in `execute` as `inputs.get('negative_prompt', '')`.
- `SdxlSampler`: defaults `steps=20`, `guidance_scale=7.5`. Resolve `seed=-1` → random. Pass `cancel_flag` as `callback_on_step_end` raising `CancelledError` if set, same pattern as `ZitSampler`.
- **Mock mode**: `SdxlLoadPipeline` → `{'pipeline': 'mock_sdxl_pipeline'}`. `SdxlTextEncode` → `{'conditioning': 'mock_sdxl_cond'}`. `SdxlSampler` → `{'latents': b'mock_sdxl_latents', 'seed': resolved}`. `SdxlDecode` → `{'image': Image.new('RGB', (64, 64), color=0)}`.
- `SdxlDecode` and `ZitDecode` both produce `{'image': PIL.Image}`. `SaveImage` in `common.py` handles both since it operates on the PIL Image generically.
- Update `worker/nodes/__init__.py` to import `sdxl`.
- Write `worker/tests/test_nodes_sdxl.py`: assert output slots; assert `SdxlTextEncode` accepts missing `negative_prompt` and defaults to `""`.

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 pytest worker/tests/test_nodes_sdxl.py -v` exits 0.

---

#### P10-A2: End-to-end cancel path — CancelJob IPC → Cancelled → DB status

**Goal:** Verify the complete cooperative cancellation flow from the HTTP endpoint through to the database status update and WebSocket broadcast.

**Files to create or modify:**
- `backend/tests/api_jobs.rs` — extend with a cancel-running-job test

**Key implementation notes:**
- The test scenario: (1) submit a mock job (it enters `Running` because the mock worker processes it immediately); (2) send `POST /v1/jobs/:id/cancel` while the job is Running; (3) assert the response is 202; (4) assert the WS stream broadcasts a `job.cancelled` event with the correct `job_id`; (5) assert `GET /v1/jobs/:id` returns `status = "Cancelled"`.
- The mock worker's `executor.py` checks `cancel_flag` between nodes. In mock mode, each node is nearly instantaneous, so the cancel may arrive after the job has already completed. To make the test deterministic, add a `ANVILML_MOCK_NODE_DELAY_MS` environment variable that inserts `time.sleep` in mock `execute()` calls, giving the test time to send the cancel before the job finishes. Set it to `200` ms in the cancel test.
- Also test the already-terminal case: attempt to cancel a `Completed` job → assert `409 job_not_cancellable`.
- The Rust scheduler's cancel logic for a Running job (P6-A4): sends `CancelJob` IPC, updates DB to `Cancelled`, broadcasts. The worker's `Cancelled` event arrives asynchronously and transitions the worker to Idle. Verify the worker reaches Idle after the cancel by asserting `GET /v1/workers` shows Idle status within 2 s.

**Acceptance criterion:** `cargo test --workspace --features mock-hardware -- cancel` exits 0.

---

#### P10-A3: Crash recovery — worker dead mid-job → Failed + respawn + new job succeeds

**Goal:** Verify the watchdog path: a worker that dies while executing a job causes the job to fail with `worker_crashed`, the worker respawns automatically, and a subsequent job succeeds.

**Files to create or modify:**
- `backend/tests/api_crash.rs` — new integration test

**Key implementation notes:**
- The test needs to force-kill a worker process from inside the Rust test. Expose a test-only method on `WorkerPool` (gated with `#[cfg(test)]`) that returns the child PID for a given worker ID: `fn get_pid_for_test(&self, worker_id: &str) -> Option<u32>`.
- Test scenario:
  1. Submit a mock job with `ANVILML_MOCK_NODE_DELAY_MS=500` so the worker stays Busy long enough.
  2. Retrieve the worker's PID and send `SIGKILL` on Unix / call `TerminateProcess` on Windows from the test.
  3. Assert the WS stream broadcasts `worker.status = Dead` within 1 s.
  4. Assert the WS stream broadcasts `job.failed { error: "worker_crashed" }` within 1 s of the Dead event.
  5. Assert the WS stream broadcasts `worker.status = Respawning` then `worker.status = Idle` within 5 s.
  6. Submit a second mock job and assert `job.completed` arrives within 5 s.
- On Windows, `TerminateProcess` requires importing the `windows` crate in the test binary. Gate with `#[cfg(windows)]` / `#[cfg(unix)]`.

**Acceptance criterion:** `cargo test --workspace --features mock-hardware -- crash` exits 0 on both Linux and Windows CI.

---

### Group B — Delivery

#### P10-B1: CI — complete openapi-diff gate and full cross-platform green

**Goal:** Confirm all CI jobs are green and `backend/openapi.json` is stable.

**Files to create or modify:**
- `.github/workflows/ci.yml` — verify all four jobs are correct and the openapi-diff step is active

**Key implementation notes:**
- The openapi-diff gate runs `cargo run -p anvilml-openapi` followed by `git diff --exit-code backend/openapi.json`. This step already exists from P7-B2; verify it is wired into the `rust` (Linux) job and that the committed `openapi.json` reflects the current handler and schema set.
- The `rust-windows` CI job runs `cargo clippy` and `cargo test --workspace --features mock-hardware`. Ensure `ANVILML_WORKER_MOCK=1` is set as an environment variable in the Windows CI job, because the worker integration tests spawn the real Python process.
- The `python-worker` CI job runs `ANVILML_WORKER_MOCK=1 pytest worker/tests/`. Confirm it covers all test files added through phase 009.
- This task is complete when the last CI run on the main branch shows all four jobs green: `rust`, `python-worker`, `rust-windows`, and the openapi-diff step within `rust`.

**Acceptance criterion:** All CI jobs green on the main branch.

---

#### P10-B2: backend/scripts — implement install_worker_deps.sh and .ps1 fully

**Goal:** Replace the stub provisioning scripts with working implementations that detect the GPU backend, create the venv, and install the correct requirements.

**Files to create or modify:**
- `backend/scripts/install_worker_deps.sh` — full Linux/macOS implementation
- `backend/scripts/install_worker_deps.ps1` — full Windows implementation

**Key implementation notes (shell):**
- Detection: `command -v nvidia-smi >/dev/null 2>&1 && BACKEND=cuda`; else `command -v rocminfo >/dev/null 2>&1 && BACKEND=rocm`; else `BACKEND=cpu`.
- Venv: prefer `uv venv --python 3.12 "${VENV_PATH:-./venv}"` if `uv` is on PATH; else `python3.12 -m venv "${VENV_PATH:-./venv}"`. Exit with a clear message if neither works.
- Install: `. "$VENV_PATH/bin/activate"` then `pip install -r worker/requirements/base.txt -q` then `pip install -r "worker/requirements/${BACKEND}.txt" -q`.
- Print: `echo "Backend: $BACKEND"; python -c "import torch; print('torch:', torch.__version__, '| CUDA available:', torch.cuda.is_available())"`.

**Key implementation notes (PowerShell):**
- Detection: `nvidia-smi` only (ROCm unavailable on Windows → always `cpu` or `cuda`).
- Venv: `uv venv --python 3.12 $VenvPath` else `py -3.12 -m venv $VenvPath`. If neither `uv` nor `py` is found, write `Write-Error` and exit 1.
- Activate: `& "$VenvPath\Scripts\Activate.ps1"`.
- Install, print: same logic as bash but PowerShell syntax.
- Both scripts must be executable (set `chmod +x` for the `.sh` in git; the `.ps1` requires `ExecutionPolicy Bypass` as documented in the usage comment).

**Acceptance criterion:** Scripts are syntactically valid (verified by `bash -n` for `.sh` and `powershell -Command "Get-Content ... | Out-Null"` for `.ps1`). Manual smoke test on a real machine confirms a working venv.

---

#### P10-B3: backend/scripts/test_inference.py — standalone debug harness

**Goal:** Implement the standalone inference debug script that runs a pipeline directly without IPC or the server, for isolating worker issues.

**Files to create or modify:**
- `backend/scripts/test_inference.py` — full replacement

**Key implementation notes:**
- CLI with `argparse`: `--model-type zit|sdxl` (required), `--model-path PATH` (required), `--prompt TEXT` (required), `--output PATH` (default `./output.png`), `--steps N` (default from `SDXL_DEFAULTS` or `ZIT_DEFAULTS`), `--seed N` (default -1), `--device cuda|rocm|cpu` (default `cpu`).
- Adds `worker/` to `sys.path` so `worker.nodes` can be imported without installing the package.
- Constructs a minimal execution context: instantiates `PipelineCache`, creates a `NodeContext` with a no-op `emit_fn` and a `threading.Event` cancel flag.
- Builds a graph dict matching the target pipeline (e.g. `ZitLoadPipeline → ZitTextEncode → ZitSampler → ZitDecode → SaveImage`) from the CLI arguments.
- Captures VRAM before and after (`torch.cuda.memory_reserved` if CUDA).
- Calls `run_graph(graph, settings, device_str, cancel_flag, emit_fn)`. The `emit_fn` prints events to stderr and saves the PNG when it receives an `ImageReady` event (by writing the decoded base64 to `--output`).
- Prints: `Elapsed: {ms} ms | VRAM before: {n} MiB | VRAM after: {n} MiB`.
- Exit 0 on success, 1 on any exception (print full traceback).

**Acceptance criterion:** `python backend/scripts/test_inference.py --model-type zit --model-path /path/to/model --prompt "test" --device cpu --output /tmp/out.png` runs without crashing on a machine with the venv provisioned and a real ZiT model file present. (Manual verification; CI does not test this script with a real model.)

---

## Phase Acceptance Criteria

```
# CI must be fully green (automated):
cargo test --workspace --features mock-hardware
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v
cargo run -p anvilml-openapi && git diff --exit-code backend/openapi.json

# Manual smoke tests (pre-release, per ANVILML_DESIGN §20.4):
# 1. Release binary starts, browser opens, system.stats ticks — Ctrl-C shuts down cleanly.
# 2. ZiT end-to-end: model in models/, submit job, progress, image in gallery, second run faster.
# 3. SDXL end-to-end: same sequence with SDXL model.
# 4. Crash recovery: kill worker, job fails, worker respawns, new job succeeds.
```

---

## Known Constraints and Gotchas

- The `ANVILML_MOCK_NODE_DELAY_MS` environment variable for the cancel test (P10-A2) is a test-only addition. Document it in `ANVILML_DESIGN.md §14.5` or a code comment so it is not accidentally removed. It must not affect production performance — only the mock executor path should read it.
- On Windows, the `api_crash.rs` test requires calling `TerminateProcess` from Rust test code. This is a `windows::Win32::System::Threading::TerminateProcess` call requiring the `windows` crate as a dev-dependency of `backend`. Gate it with `#[cfg(windows)]` and skip the PID-kill approach on Linux (use `nix::sys::signal::kill` or `libc::kill` instead). Both paths exercise the same Rust watchdog logic.
- `install_worker_deps.sh` assumes `python3.12` is available by that exact name on Linux. On some distributions the command is `python3` or `python`. The script should check `python3.12` first, then fall back to `python3.12` via `uv`. If neither works, exit 1 with `"Python 3.12 not found. Install it or use uv."`.
- The `test_inference.py` debug harness runs in the same process as the model — if the model causes an OOM crash, the script itself crashes. This is intentional; the script is a debug tool, not a production code path. The crash will print a full traceback for diagnosis.
- `backend/openapi.json` must be committed with the final schema for the openapi-diff gate to pass. After completing P7-B2, regenerate it with `cargo run -p anvilml-openapi` and commit the result. Any subsequent handler or type change that diverges from the committed file will fail CI, which is the intended behaviour.
