# Plan Report: P17-D1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P17-D1                                      |
| Phase       | 17 — Cancellation                           |
| Description | Runnable Proof: cancelling a Queued job returns 202 then 409 on retry |
| Depends on  | P17-C1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-11T18:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Run the AnvilML binary (built with `mock-hardware` feature) as a background process, submit a single-node `PassThrough` job via `POST /v1/jobs`, immediately cancel it via `POST /v1/jobs/:id/cancel` and assert HTTP 202 (the success path for a cancellable Queued job), then cancel the same now-`Cancelled` job again and assert HTTP 409 (the idempotent-cancel rejection). Record the literal terminal output in the implementation report. No source files are created or modified — this task exercises the live server's cancellation HTTP endpoint.

## Scope

### In Scope
- Build the AnvilML binary with `--features mock-hardware` (release profile).
- Launch the binary in the background with `ANVILML_MOCK_NODE_DELAY_MS` set high enough to keep the job observable in a non-terminal state.
- Submit a single-node `PassThrough` job via `POST /v1/jobs`.
- Immediately cancel the submitted job via `POST /v1/jobs/:id/cancel` and assert HTTP 202.
- Cancel the same job ID again and assert HTTP 409.
- Kill the background server process.
- Record the literal terminal output in the implementation report.

### Out of Scope
None. `defers_to (from JSON): []` — this task has no deferrals and must implement its full scope as described.

## Existing Codebase Assessment

The cancellation infrastructure is fully implemented across Phase 17's prior tasks:

**(a) What already exists:**
- `JobScheduler::cancel()` in `crates/anvilml-scheduler/src/scheduler.rs` (lines 360–486) implements full status-aware branching: Queued jobs are lazily removed from the in-memory queue and their DB status is set to `Cancelled` immediately; Running jobs receive a cooperative `CancelJob` IPC signal via `WorkerMessage::CancelJob`; terminal jobs (`Completed`/`Failed`/`Cancelled`) return `CancelOutcome::AlreadyTerminal`; unknown IDs return `CancelOutcome::NotFound`.
- The HTTP handler `cancel_job()` in `crates/anvilml-server/src/handlers/jobs.rs` (lines 183–207) maps `CancelOutcome::Accepted` → 202, `AlreadyTerminal` → 409, `NotFound` → 404.
- The `CancelOutcome` enum in `scheduler.rs` (lines 72–80) provides the three-way distinction the handler needs.
- Integration tests for all three cancel paths exist in `crates/anvilml-server/tests/jobs_tests.rs`: `test_cancel_queued_job_returns_202` (line 558), `test_cancel_completed_job_returns_409` (line 617), `test_cancel_unknown_id_returns_404` (line 682), `test_cancel_running_job_returns_202` (line 703), and `test_cancel_already_cancelled_job_returns_409` (line 772).

**(b) Established patterns:**
- The build command uses `cargo build --release -p anvilml --features mock-hardware`.
- The binary listens on `127.0.0.1:8488` by default.
- Mock hardware mode is driven by `mock-hardware` cargo feature + `ANVILML_MOCK_*` env vars.
- The `ANVILML_MOCK_NODE_DELAY_MS` env var controls artificial delay in the Python worker's mock node execution, keeping jobs observable in intermediate states.

**(c) Gap between design doc and current source:** None. The cancellation flow (Queued→202, AlreadyTerminal→409, NotFound→404) is fully implemented and tested. The Runnable Proof's acceptance criterion matches the existing implementation exactly.

## Resolved Dependencies

None. This task does not introduce or reference any external crates or packages. It exercises the already-built binary via shell commands (curl, kill).

## Approach

### Step 1 — Build the binary
Run `cargo build --release -p anvilml --features mock-hardware`. This produces `target/release/anvilml`. The release build is required because the Runnable Proof is an end-to-end live test, and the mock-hardware feature must be compiled in.

### Step 2 — Launch the server in background
Launch the binary with `ANVILML_MOCK_NODE_DELAY_MS` set to a value high enough (e.g., `30000` ms = 30 seconds) to ensure the job remains in `Queued`/`Running` state long enough for the cancel request to arrive. The `ANVILML_MOCK_NODE_DELAY_MS` env var is read by the Python worker's mock nodes — it creates an artificial delay per node execution, which means a single-node `PassThrough` job will stay `Running` for at least that duration. This prevents the job from completing before the first cancel call.

Launch command:
```bash
ANVILML_MOCK_NODE_DELAY_MS=30000 ./target/release/anvilml &
SERVER_PID=$!
sleep 2
```

The 2-second sleep allows the server to bind the HTTP port and complete startup (hardware detection, worker pool spawn, SQLite migrations).

### Step 3 — Submit a job
Submit a single-node `PassThrough` job:
```bash
JOB_ID=$(curl -s -X POST http://127.0.0.1:8488/v1/jobs \
  -H 'Content-Type: application/json' \
  -d '{"graph":{"nodes":[{"id":"n0","type":"PassThrough","inputs":{"value":1}}]},"settings":{}}' \
  | python3 -c "import sys,json;print(json.load(sys.stdin)['job_id'])")
```

The `PassThrough` node type must be registered by a worker. In mock-hardware mode, the worker starts quickly and sends a `Ready` event registering available node types including `PassThrough`. The `sleep 2` after launch should be sufficient for this registration.

### Step 4 — First cancel (expect 202)
Cancel the job immediately:
```bash
curl -s -o /dev/null -w '%{http_code}' -X POST "http://127.0.0.1:8488/v1/jobs/$JOB_ID/cancel"
```
Assert output is `202`. This exercises the Queued→Cancelled path in `JobScheduler::cancel()`, which lazily removes the job from the in-memory queue and persists the `Cancelled` status.

### Step 5 — Second cancel (expect 409)
Cancel the same job ID again:
```bash
curl -s -o /dev/null -w '%{http_code}' -X POST "http://127.0.0.1:8488/v1/jobs/$JOB_ID/cancel"
```
Assert output is `409`. This exercises the AlreadyTerminal→409 path: the job is now in `Cancelled` status (terminal), so `cancel()` returns `CancelOutcome::AlreadyTerminal`, which the handler maps to HTTP 409.

### Step 6 — Cleanup
Kill the background server:
```bash
kill "$SERVER_PID" 2>/dev/null
wait "$SERVER_PID" 2>/dev/null
```

## Public API Surface

None. This task does not introduce or modify any source code, pub items, or API surfaces.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| (none) | — | No source files created or modified. This is a Runnable Proof task. |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| (Runnable Proof) | cancel_queued_returns_202 | POST /v1/jobs/:id/cancel on a Queued job returns HTTP 202 | Server running with mock-hardware; PassThrough node registered | Single-node PassThrough job graph | HTTP 202 | `curl -s -o /dev/null -w '%{http_code}' -X POST "http://127.0.0.1:8488/v1/jobs/$JOB_ID/cancel" → 202` |
| (Runnable Proof) | cancel_already_cancelled_returns_409 | POST /v1/jobs/:id/cancel on an already-Cancelled job returns HTTP 409 | Same job ID from previous step (now Cancelled) | Same job ID | HTTP 409 | `curl -s -o /dev/null -w '%{http_code}' -X POST "http://127.0.0.1:8488/v1/jobs/$JOB_ID/cancel" → 409` |

## CI Impact

No CI changes required. This task does not modify any CI workflow files, test files, or build configurations. The Runnable Proof is a manual test executed by the agent, not a CI job.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The build command (`cargo build --release -p anvilml --features mock-hardware`) is cross-platform. The curl commands use POSIX shell syntax; the proof is designed for Linux/WSL2 execution.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The server may not have started within the 2-second sleep window, causing the job submission to fail with a connection error. | Low | Medium | Increase sleep to 3 seconds if needed. Check server readiness by attempting a GET /health first before submitting the job. |
| The Python mock worker may not have registered the PassThrough node type within the startup window, causing graph validation to reject the job (400 Bad Request). | Low | Medium | The PassThrough node is a core node that should be registered by the worker's Ready event. If it fails, increase startup delay or verify the node is registered by checking GET /v1/nodes first. |
| `ANVILML_MOCK_NODE_DELAY_MS` may not propagate to the mock worker correctly, causing the job to complete before the cancel call arrives. | Medium | High | Set the delay to a very high value (30000ms = 30s) to provide ample margin. If the job still completes too fast, verify the env var is being read by the mock node's execute() method. |
| The server process may not terminate cleanly with `kill`, leaving the port bound for subsequent runs. | Low | Low | Use `kill "$SERVER_PID" 2>/dev/null` followed by `wait "$SERVER_PID" 2>/dev/null`. If the port is still in use, add `sleep 1` before the next test run. |

## Acceptance Criteria

- [ ] `cargo build --release -p anvilml --features mock-hardware` exits 0
- [ ] `ANVILML_MOCK_NODE_DELAY_MS=30000 ./target/release/anvilml &` starts server (background process exists)
- [ ] `curl -s http://127.0.0.1:8488/health` returns 200 (server is ready)
- [ ] `curl -s -X POST http://127.0.0.1:8488/v1/jobs ... | python3 -c "..."` extracts a non-empty job_id
- [ ] First `POST /v1/jobs/$JOB_ID/cancel` returns HTTP 202
- [ ] Second `POST /v1/jobs/$JOB_ID/cancel` returns HTTP 409
- [ ] `kill "$SERVER_PID"` terminates the server process
