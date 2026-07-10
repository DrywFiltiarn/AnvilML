# Plan Report: P16-E1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P16-E1                                            |
| Phase       | 16 — Live Events                                  |
| Description | Runnable Proof: WebSocket client observes JobCompleted for PassThrough job |
| Depends on  | P16-D1                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-07-10T22:58:00Z                              |
| Attempt     | 1                                                 |

## Objective

Produce Phase 16's Runnable Proof: build the AnvilML binary with `mock-hardware`, start it as a background process, connect a Python WebSocket client to `ws://127.0.0.1:8488/v1/events`, submit a single-node PassThrough job via `POST /v1/jobs`, and assert that a `job_completed` JSON frame carrying the matching `job_id` arrives on the WebSocket within 10 seconds. This is the first phase where the live event stream (`GET /v1/events`) is exercised end-to-end against real dispatch — not just REST polling of job status as Phase 14's proof did.

## Scope

### In Scope
- Create a short Python script (`scripts/run_proof_p16_e1.py`) using the `websockets` library that:
  - Connects to `ws://127.0.0.1:8488/v1/events`.
  - Consumes the initial `SystemStats` frame.
  - Submits a PassThrough job via parallel HTTP POST to `http://127.0.0.1:8488/v1/jobs`.
  - Reads from the WebSocket until a `job_completed` JSON frame with the matching `job_id` arrives.
  - Times out after 10 seconds if no matching frame arrives.
  - Prints the received frame to stdout.
  - Exits 0 on success, non-zero on failure.
- Verify the script passes end-to-end when the server is running under mock-hardware.
- Record the literal terminal output in the implementation report.

### Out of Scope
- No new source files or Rust/Python code changes. This task is purely a proof script + execution.
- No changes to the server binary, WebSocket handler, event loop, or node system.
- No real-mode (GPU/torch) testing — mock-hardware only.

## Existing Codebase Assessment

The Phase 16 source code is fully implemented across tasks P16-A1 through P16-D1. The key components relevant to this proof are:

1. **WebSocket handler** (`crates/anvilml-server/src/ws/handler.rs`): A thin `ws_handler()` function that upgrades the connection, subscribes to the shared `EventBroadcaster`, sends an initial `SystemStats` frame (placeholder zero-valued), then enters a forward loop that serialises every `WsEvent` as JSON text and sends it to the client. On `Lagged` disconnect, it sends a Close frame and breaks.

2. **Event broadcaster** (`crates/anvilml-ipc/src/ws/broadcaster.rs`): A `tokio::sync::broadcast` wrapper. The same `Arc<EventBroadcaster>` instance is shared between the scheduler's event loop (`spawn_event_loop`) and `AppState`, so HTTP-layer WebSocket subscribers see all events the scheduler publishes.

3. **Event loop** (`crates/anvilml-scheduler/src/event_loop.rs`): Consumes `WorkerEvent`s from `Demux::subscribe()`, maps each to a `WsEvent` variant via `map_worker_event()`, and publishes via the broadcaster. Terminal events (`Completed`, `Failed`, `Cancelled`) persist status to the database, release VRAM reservations, restore workers to `Idle`, and wake the dispatch loop — all before publishing the mapped `WsEvent`.

4. **Stats tick** (`crates/anvilml-server/src/ws/stats_tick.rs`): Publishes `WsEvent::SystemStats` every 5 seconds to all connected clients.

5. **PassThrough node** (`worker/nodes/passthrough.py`): The simplest concrete node — `NODE_TYPE="PassThrough"`, takes a single `value` input of type `ANY`, returns the same value unchanged. Both mock and real branches return the input identically.

6. **Existing handler tests** (`crates/anvilml-server/tests/handler_tests.rs`): 8 integration tests using `tokio_tungstenite` that verify the WebSocket handler's connect sequence, initial frame, forward loop, multi-client support, and lagged disconnect. These establish the test patterns used throughout the project.

The established patterns are: `serde_json` for JSON serialisation, `tracing` for logging, structured error handling, and the `#[tracing::instrument]` attribute on async functions. No dual-mode parity markers apply to this task (it creates no source code).

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source     | Feature flags confirmed |
|--------|-----------|-----------------|----------------|------------------------|
| python | websockets| 16.0            | pypi-query MCP | n/a (async context manager API) |

Note: `pypi-query` reported 16.1 as the latest available version; the installed venv carries 16.0. Both use the same async context manager API (`async with websockets.connect(...) as ws:`), so this is not a concern. The `websockets` library requires Python ≥3.10; the project uses Python 3.12.x.

## Approach

### Phase Deliverable Audit (§9a, §9a.1, §9a.2)

This is the phase-closing task (last entry in `tasks_phase016.json`). The following audits were run:

**§9a — Defers-to audit:** Two tasks in this phase have non-empty `defers_to`:
- `P16-A2` → `P16-A3` (worker Idle restoration + dispatch wake deferred to A3)
- `P16-C1` → `P16-C2` (forward loop deferred to C2)

Both P16-A3 and P16-C2 are downstream tasks in the phase. The `defers_to` comment markers (`// defers_to: P16-A3` / `// defers_to: P16-C2`) were checked in the relevant source files:
```bash
grep -rn "defers_to:" crates/anvilml-scheduler/src/event_loop.rs crates/anvilml-server/src/ws/handler.rs
```
Result: 0 findings. Neither file carries a `defers_to:` comment because the deferred scope was fully implemented (not stubbed) — P16-A3's worker Idle restoration and dispatch wake are fully present in `event_loop.rs` (lines 422-454 for Completed, 503-519 for Failed, 570-586 for Cancelled), and P16-C2's forward loop is fully present in `handler.rs` (lines 69-106). No stub sites exist, so no `defers_to:` markers are expected.

**§9a.1 — Unmarked-stub sweep:**
```bash
grep -rn "NotImplementedError\|unimplemented!\|todo!\|# TODO\|// TODO" crates/ worker/ --include='*.rs' --include='*.py'
```
Result: 0 findings in project source files (all matches were in `worker/.venv/` third-party packages: torch, pip, zmq). No unmarked stubs present.

**§9a.2 — Dual-mode parity-marker sweep:**
```bash
grep -L "REAL_PATH_VERIFIED:" worker/nodes/**/*.py | grep -v __init__ | grep -v base.py
grep -L "MOCK_PATH_VERIFIED:" worker/nodes/**/*.py | grep -v __init__ | grep -v base.py
```
Result: 0 findings for both commands. All node files carry both markers.

**Audit summary:** Phase deliverable audit passes. `defers_to` entries are accounted for (scope fully implemented, no stubs), unmarked-stub sweep: 0 findings, dual-mode parity-marker sweep: 0 findings.

### Step 1 — Build the binary

Run `cargo build --release -p anvilml --features mock-hardware` from the workspace root. This compiles the full server binary with mock hardware detection and mock worker injection. The `mock-hardware` feature replaces GPU detection with `MockDetector` driven by `ANVILML_MOCK_*` env vars, and causes the supervisor to inject `ANVILML_WORKER_MOCK=1` into spawned worker subprocesses.

### Step 2 — Start the server

Launch the binary as a background process:
```bash
./target/release/anvilml &
SERVER_PID=$!
```

Wait for the server to become ready (the `/health` endpoint responding with HTTP 200). A `sleep 2` is sufficient given the mock-hardware startup path has no real hardware probe or model scan.

### Step 3 — Create the proof script

Write `scripts/run_proof_p16_e1.py` — a self-contained Python script that:

1. **Connects** to `ws://127.0.0.1:8488/v1/events` using `async with websockets.connect(...) as ws:`.
2. **Consumes the initial frame** via `ws.recv()` and asserts it is a `system_stats` type (the connection's first message per `ANVILML_DESIGN.md §13.6`).
3. **Submits a PassThrough job** in parallel via `urllib.request` POST to `http://127.0.0.1:8488/v1/jobs` with:
   ```json
   {"graph": {"nodes": [{"id": "n0", "type": "PassThrough", "inputs": {"value": 1}}]}, "settings": {}}
   ```
   Extracts the `job_id` from the `202 Accepted` response.
4. **Reads the WebSocket** in a loop, deserialising each frame as JSON, checking if `frame["type"] == "job_completed"` and `frame["job_id"] == job_id`. On match, prints the frame and returns.
5. **Times out** after 10 seconds using `asyncio.timeout(10)`. On timeout, prints an error with the last N frames received and exits non-zero.

The script uses `asyncio` for the WebSocket side and `urllib.request` (synchronous, from stdlib) for the HTTP POST — no additional Python dependencies beyond `websockets`.

### Step 4 — Execute the proof

Run the script:
```bash
python3 scripts/run_proof_p16_e1.py
```

Expected outcome: the script prints the `job_completed` JSON frame to stdout and exits 0. The frame should contain `type: "job_completed"`, the submitted `job_id`, and an `elapsed_ms` value.

### Step 5 — Clean up

Kill the background server process:
```bash
kill "$SERVER_PID" 2>/dev/null
```

## Public API Surface

None. This task introduces no new source code, no new pub items, no new types or functions. It creates a single Python script file for the proof.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `scripts/run_proof_p16_e1.py` | Runnable proof script: WebSocket client observes JobCompleted for PassThrough job |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `scripts/run_proof_p16_e1.py` | proof_ws_job_completed_pass_through | WebSocket client receives JobCompleted for a PassThrough job within 10s | Server running with mock-hardware; workers spawned and Ready; PassThrough node registered | PassThrough job submitted via POST /v1/jobs | JSON frame with type=job_completed and matching job_id printed to stdout; script exits 0 | `python3 scripts/run_proof_p16_e1.py` exits 0 |

## CI Impact

No CI changes required. This task creates a single Python script under `scripts/` that is executed manually as part of the Runnable Proof. It does not add test files, CI configuration, or change any existing CI job's behavior. The proof is a manual verification step (tagged `"manual"` in the task definition), not an automated CI test.

## Platform Considerations

None identified. The proof script runs on the host platform (Linux/WSL2) and connects to `127.0.0.1:8488` — a loopback address that works identically on Linux and Windows. The `websockets` library is cross-platform. The server binary is built with `--features mock-hardware` which is the same feature flag used across all CI builds. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed since no Rust code is modified.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Server startup takes longer than expected (worker spawn delay, model scan), causing the HTTP POST to fail with connection refused | Low | Medium | The script should poll `/health` in a loop before attempting the job submission, with a 30-second timeout. This ensures the server is fully ready (workers spawned, Ready events processed, node registry populated) before submitting the job. |
| The `websockets` 16.x API differs from the `async with websockets.connect()` pattern used in `docs/RUNNABLE_PROOF.md` | Low | Medium | The MCP lookup confirmed `websockets.connect()` is an async context manager in 16.x, and `ClientConnection` has `recv()`/`send()` methods. The script will be tested against the actual installed version. |
| The job completes so quickly that the WebSocket `recv()` races with the HTTP POST (the event is published before the client starts listening) | Low | Low | The initial `SystemStats` frame is consumed first, then the HTTP POST is made. The event loop only publishes `JobCompleted` after the worker finishes execution, which takes at least the mock node delay. Even without a delay, the HTTP POST is issued before the WebSocket loop begins waiting for events, so the timing window is negligible. |
| Mock worker never reaches Ready state, causing job submission to return 503 workers_unavailable | Low | High | This would indicate a deeper issue with the Phase 16 implementation (worker spawn, Ready event routing, node registry). The proof would fail with a clear error message, and the ACT agent would need to diagnose the root cause. |

## Acceptance Criteria

- [ ] `cargo build --release -p anvilml --features mock-hardware` exits 0
- [ ] `python3 scripts/run_proof_p16_e1.py` exits 0 after printing a `job_completed` frame with the matching `job_id`
- [ ] The `job_completed` frame contains `"type": "job_completed"` and the `job_id` matches the one returned by `POST /v1/jobs`
- [ ] The entire proof (server start → job submit → event received) completes within 15 seconds
