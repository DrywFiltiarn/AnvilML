# Plan Report: P18-G1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-G1                                      |
| Phase       | 18 — HTTP/WebSocket Server Completion       |
| Description | Runnable Proof: live binary serves /v1/system and /v1/workers with real data |
| Depends on  | P18-F2                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-12T18:20:00Z                        |
| Attempt     | 1                                           |

## Objective

Produce Phase 18's Runnable Proof by building the `anvilml` binary with `--features mock-hardware`, launching it with `ANVILML_MOCK_DEVICE_TYPE=cuda`, and confirming via HTTP that `GET /v1/system` returns 200 with at least one GPU entry (from the mock detector) and `GET /v1/workers` returns 200 with a JSON array. This validates that the full REST surface from `ANVILML_DESIGN.md §13.4` is now backed by real, non-stub logic across all eighteen phases. No new source files are created.

## Scope

### In Scope
- Build the release binary: `cargo build --release -p anvilml --features mock-hardware`
- Launch the binary in the background with `ANVILML_MOCK_DEVICE_TYPE=cuda`
- Send `GET /v1/system` and assert the response body contains `gpus` array with `>=1` entry
- Send `GET /v1/workers` and assert the response body is a JSON array (possibly empty — workers may still be initializing)
- Kill the background process after verification
- Record the literal terminal output in the implementation report

### Out of Scope
None. This task's `defers_to` field is empty (`[]`), and the task context requires implementing its full scope. No functionality is deferred.

## Existing Codebase Assessment

The codebase inspection confirms all infrastructure required by this Runnable Proof is already in place:

**(a) What exists:** The `GET /v1/system` handler (`handlers/system.rs:get_system`) is a thin delegation that read-locks `AppState.hardware` (an `Arc<RwLock<HardwareInfo>>`) and returns the clone as JSON. The handler is registered in `lib.rs:build_router()` at route `/v1/system`. The `HardwareInfo` type (in `anvilml-core/src/types/hardware.rs`) contains `gpus: Vec<GpuDevice>` — the field the proof asserts on. The `GET /v1/workers` handler (`handlers/workers.rs:list_workers`) delegates to `WorkerPool::list()` which returns `Vec<WorkerInfo>`, registered at route `/v1/workers`. Both handlers are annotated with `#[utoipa::path(...)]` for OpenAPI generation (P18-F1).

**(b) Established patterns:** Handlers are one-line delegations with no business logic (per `ANVILML_DESIGN.md §3.3`). The mock-hardware feature replaces `anvilml-hardware`'s real detectors with `MockDetector`, which returns synthetic GPU devices driven by `ANVILML_MOCK_*` env vars. The binary's main.rs detects hardware at startup (line 179), logs device count, and stores the snapshot in `AppState.hardware`. WorkerPool is constructed and workers are spawned before the router is built.

**(c) Gap between design doc and source:** None. The design doc specifies `GET /v1/system` returns `HardwareInfo` and `GET /v1/workers` returns `Vec<WorkerInfo>` — both match the actual handler implementations exactly. The mock-hardware feature flag is correctly wired: `ANVILML_MOCK_DEVICE_TYPE=cuda` will produce one mock GPU device with `enumeration_source: Mock` and `capabilities_source: Fallback`.

## Resolved Dependencies

None. This task introduces no new dependencies — it runs the already-built binary. All dependencies (axum, tokio, serde, utoipa, rmp-serde, zeromq) were resolved in prior phases.

## Approach

### Step 1: Build the release binary

Run `cargo build --release -p anvilml --features mock-hardware` from the repository root. The `mock-hardware` feature replaces `anvilml-hardware`'s Vulkan/DXGI/sysfs detectors with `MockDetector`, which reads `ANVILML_MOCK_DEVICE_TYPE` and related env vars to synthesize GPU device info. This is the same feature flag used by all CI builds.

### Step 2: Launch the binary in mock mode

Set `ANVILML_MOCK_DEVICE_TYPE=cuda` and run the built binary as a background process:
```bash
ANVILML_MOCK_DEVICE_TYPE=cuda ./target/release/anvilml &
SERVER_PID=$!
```

The binary will:
1. Parse CLI args and load config (defaults apply since no `anvilml.toml` is needed for mock mode)
2. Create the SQLite database pool and run migrations
3. Run the device capabilities seed loader
4. Call `detect_all_devices()` — with mock-hardware active, `MockDetector` will create one synthetic GPU based on `ANVILML_MOCK_DEVICE_TYPE=cuda`
5. Construct `WorkerPool` and attempt to spawn workers (these will fail to connect to a Python interpreter, but the server continues — workers will remain in `Initializing` state)
6. Build the router and bind the TCP listener on `127.0.0.1:8488`

### Step 3: Wait for the server to be ready

Sleep for 2 seconds to allow the server to bind the listener and complete startup. This is the same wait time specified in the Runnable Proof acceptance criterion.

### Step 4: Verify `GET /v1/system` returns 200 with GPU data

Send `curl -s http://127.0.0.1:8488/v1/system` and pipe through Python to parse the JSON response:
```bash
curl -s http://127.0.0.1:8488/v1/system | python3 -c "import sys,json; d=json.load(sys.stdin); assert len(d['gpus'])>=1"
```

Expected behavior: The mock detector produced one GPU device (since `ANVILML_MOCK_DEVICE_TYPE=cuda`), so `d['gpus']` should contain at least one entry. The assertion passes if `len(d['gpus']) >= 1`.

### Step 5: Verify `GET /v1/workers` returns a JSON array

Send `curl -s http://127.0.0.1:8488/v1/workers` and pipe through Python:
```bash
curl -s http://127.0.0.1:8488/v1/workers | python3 -c "import sys,json; assert isinstance(json.load(sys.stdin), list)"
```

Expected behavior: The handler returns `Json(state.workers.list().await)` which always returns a `Vec<WorkerInfo>`. If workers failed to spawn (likely in this environment without a Python venv), the vector is empty `[]`, which is still a valid JSON array. The assertion passes for both empty and non-empty arrays.

### Step 6: Kill the background process

```bash
kill "$SERVER_PID" 2>/dev/null
```

### Phase Deliverable Audit

This is the phase-closing task (confirmed: P18-G1 is the last task in `tasks_phase018.json`). Per FORGE_AGENT_RULES.md §9a, §9a.1, and §9a.2, the following audits were run:

**§9a — defers_to coverage audit:**
Tasks with non-empty `defers_to` in this phase:
- P18-C1 → `["P18-C2"]`: P18-C2's description is "anvilml-server: POST /v1/models/rescan handler" — P18-C1 defers the rescan handler to P18-C2. Verified: P18-C2's context states it implements `rescan_models()`.
- P18-D1 → `["P18-D3"]`: P18-D1 defers `POST /v1/workers/:id/restart` to P18-D3. Verified: P18-D3's description is "anvilml-server: POST /v1/workers/:id/restart via explicit respawn".
- P18-E1 → `["P18-E2"]`: P18-E1 defers bulk clear to P18-E2. Verified: P18-E2's description is "anvilml-server: DELETE /v1/jobs bulk clear handler".

All three defers_to entries name tasks whose descriptions genuinely cover the deferred scope.

**§9a.1 — Unmarked-stub sweep:**
```bash
grep -rn "NotImplementedError\|unimplemented!\|todo!\|# TODO\|// TODO" backend/src/ crates/anvilml-server/src/handlers/ crates/anvilml-worker/src/ crates/anvilml-server/src/
```
Result: 0 findings. No unmarked stubs exist in any source file modified by this phase.

**§9a.2 — Dual-mode parity-marker sweep:**
The project defines the `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` marker convention (`ANVILML_DESIGN.md §10.6`). This convention applies to node and arch-module functions in `worker/nodes/`, not to server handlers. The sweep greps:
```bash
grep -L "REAL_PATH_VERIFIED:" worker/nodes/**/*.py | grep -v __init__ | grep -v base.py
grep -L "MOCK_PATH_VERIFIED:" worker/nodes/**/*.py | grep -v __init__ | grep -v base.py
```
Result: Both commands returned empty (all node files have both markers). 0 findings.

This task (P18-G1) does not add or modify any node function or arch-module function — it only verifies HTTP endpoints served by existing handler code. The dual-mode parity-marker convention does not apply to handler functions, so no markers are relevant here.

## Public API Surface

None. This task does not introduce or modify any public API items. It exercises existing public routes (`GET /v1/system`, `GET /v1/workers`) that were implemented in prior phase tasks (P18-B1, P18-D1).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| No change | (none) | This task runs the already-built binary; no source files are created or modified |

## Tests

This task is a Runnable Proof with no new source code. The acceptance criteria are validated by the literal terminal output recorded in the implementation report. No new test files or test functions are created.

The existing test suites that cover the exercised handlers are:
- `crates/anvilml-server/tests/system_tests.rs` — tests for `GET /v1/system` (P18-B1/B2)
- `crates/anvilml-server/tests/workers_tests.rs` — tests for `GET /v1/workers` (P18-D1) and `POST /v1/workers/:id/restart` (P18-D3)

These tests already verify the handler logic in-process. The Runnable Proof exercises the same code through a real HTTP server, providing end-to-end integration verification.

## CI Impact

No CI changes required. This task does not modify any CI workflow files, test files, or build configuration. The `openapi-drift` CI gate (P18-F2) was the prerequisite that added the real gate — P18-G1 simply exercises the resulting live binary.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The mock-hardware feature produces platform-neutral results (synthetic GPU data), and the HTTP server binds on `127.0.0.1:8488` identically on Linux and Windows. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are relevant to this task.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The server fails to bind port 8488 because it's already in use from a prior run | Low | Medium — the proof fails even though the code is correct | The proof script uses `kill "$SERVER_PID" 2>/dev/null` which cleans up any prior process. If port is still bound, the `sleep 2` should be extended or the port checked first. |
| Python subprocess (`python3 -c "..."`) is not available in the environment | Low | High — the proof cannot validate JSON responses | Use `jq` as an alternative: `curl -s http://127.0.0.1:8488/v1/system | jq '.gpus | length >= 1'`. Document the alternative in the report if needed. |
| Worker spawn fails at startup (no Python venv) and the server still starts | Medium | Low — this is expected behavior; workers remain in Initializing state | The proof only checks `/v1/system` (hardware) and `/v1/workers` (empty or initializing list). Worker spawn failure does not affect these endpoints. |
| The mock detector returns zero GPUs despite `ANVILML_MOCK_DEVICE_TYPE=cuda` | Low | High — the assertion `len(d['gpus'])>=1` would fail | Check the `MockDetector` implementation in `crates/anvilml-hardware/src/mock.rs` to confirm it creates one device per mock env var. If it doesn't, this is a code defect to report as a blocker. |

## Acceptance Criteria

- [ ] `cargo build --release -p anvilml --features mock-hardware` exits 0
- [ ] `ANVILML_MOCK_DEVICE_TYPE=cuda ./target/release/anvilml &` starts the server (background process)
- [ ] `curl -s http://127.0.0.1:8488/v1/system | python3 -c "import sys,json; d=json.load(sys.stdin); assert len(d['gpus'])>=1"` exits 0
- [ ] `curl -s http://127.0.0.1:8488/v1/workers | python3 -c "import sys,json; assert isinstance(json.load(sys.stdin), list)"` exits 0
- [ ] `kill "$SERVER_PID" 2>/dev/null` terminates the background process
