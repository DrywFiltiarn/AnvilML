# Plan Report: P14-E1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P14-E1                                      |
| Phase       | 014 — Dispatch & Execute                    |
| Description | Runnable Proof: submitted job with PassThrough node reaches Completed |
| Depends on  | P14-D2                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-08T12:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Produce Phase 14's Runnable Proof: build the AnvilML binary with `mock-hardware`, start it as a background process, submit a single-node computation graph (the real `PassThrough` node from P14-B1) via `POST /v1/jobs`, poll `GET /v1/jobs/:id` until the job status leaves `Queued`/`Running` and reaches `Completed`, and record the literal terminal output. This is the first genuine end-to-end demonstration of real dispatch against a real node — not a mocked stand-in for any link in the chain. No source files are created or modified.

## Scope

### In Scope
- Build the release binary: `cargo build --release -p anvilml --features mock-hardware`
- Start the binary as a background process on the default `127.0.0.1:8488`
- Submit a single-node `PassThrough` graph via `POST /v1/jobs`
- Poll `GET /v1/jobs/:id` until the job reaches a terminal status (`Completed` or `Failed`)
- Assert the job status is `Completed`
- Record the literal terminal output for the implementation report
- Kill the background process after completion

### Out of Scope
None. This task's `defers_to` field is `[]` (empty) — no scope is deferred. The acceptance criterion is a single self-contained bash command that builds, runs, submits, polls, asserts, and cleans up in sequence.

## Existing Codebase Assessment

The codebase at this point has the full Phase 14 pipeline assembled:

**(a) What exists:** The `backend/src/main.rs` already constructs a `WorkerPool`, spawns workers via `spawn_all()`, constructs a `JobScheduler` with its dispatch loop, and builds `AppState` with all required fields (`scheduler`, `workers`, `db`). The `anvilml-server` crate has `submit_job()`, `list_jobs()`, and `get_job()` handlers in `handlers/jobs.rs`. The `PassThrough` node exists in `worker/nodes/passthrough.py` with both mock and real `execute()` branches and dual-mode parity markers. The node registry auto-imports `passthrough.py` at package load time, so it is registered before any job submission.

**(b) Established patterns:** Error handling uses `AnvilError` with `IntoResponse` for HTTP status mapping (e.g., `WorkersUnavailable → 503`, `InvalidGraph → 400`). The scheduler's `submit()` validates the graph, persists the job, enqueues it, and returns `(job_id, queue_position)`. The dispatch loop polls the queue on `Notify` and dispatches `WorkerMessage::Execute` to an idle worker. Job status transitions (`Queued → Running → Completed`) are handled by the event loop subscriber.

**(c) Gap analysis:** There is no gap between the design doc and current source for this task. The entire pipeline — from HTTP handler through scheduler dispatch to worker execution and status update — is already implemented by prior Phase 14 tasks. This task exercises it as a whole.

## Resolved Dependencies

None. This task introduces no new crates, packages, or dependencies. It only runs the already-built binary and uses standard tools (`curl`, `python3`) for HTTP interaction.

| Type | Name | Version verified | MCP source | Feature flags confirmed |
|------|------|-----------------|------------|------------------------|

## Approach

### Phase Deliverable Audit (mandatory for phase-closing task per §9a)

Per `FORGE_AGENT_RULES.md §9a`, §9a.1, and §9a.2, the following audits were run before writing this plan:

**§9a — defers_to coverage:** Ran `grep -rn "defers_to:" .forge/tasks/tasks_phase014.json` — produced no output. No task in Phase 14 has a non-empty `defers_to` field. No ownership links to verify.

**§9a.1 — Unmarked-stub sweep:** Ran `grep -rn "NotImplementedError\|unimplemented!\|todo!\|# TODO\|// TODO" worker/nodes/ crates/ backend/src/` — `0 findings`. No unmarked stubs in any source file modified by Phase 14 tasks.

**§9a.2 — Dual-mode parity-marker sweep:** Ran:
```
grep -L "REAL_PATH_VERIFIED:" worker/nodes/**/*.py | grep -v __init__ | grep -v base.py
grep -L "MOCK_PATH_VERIFIED:" worker/nodes/**/*.py | grep -v __init__ | grep -v base.py
```
Both commands produced empty output — `0 findings`. Every node file defines both markers.

**Result:** Phase 14 passes all three audit checks. No blockers from the phase-closing audit.

### Step 1 — Build the release binary

Run:
```bash
cargo build --release -p anvilml --features mock-hardware
```

This compiles the full workspace with the `mock-hardware` feature, producing `target/release/anvilml`. The `mock-hardware` feature replaces GPU detection with `MockDetector` (driven by `ANVILML_MOCK_*` env vars) and causes the Python worker subprocess to receive `ANVILML_WORKER_MOCK=1` in its environment.

**Rationale:** The acceptance criterion specifies `--release` for a realistic build that exercises the same code paths as production. The `mock-hardware` feature is required because this environment has no real GPU hardware.

### Step 2 — Start the server

Run the binary in the background:
```bash
./target/release/anvilml &
```

The server starts, detects mock hardware, spawns one Python worker subprocess (with `ANVILML_WORKER_MOCK=1`), the worker imports `worker.nodes.passthrough` (triggering `@register`), sends a `Ready` event with `PassThrough` in its node types, and the scheduler's dispatch loop starts. The server binds to `127.0.0.1:8488`.

**Rationale:** No config overrides are needed — the checked-in `anvilml.toml` at the repo root already sets `host = "127.0.0.1"` and `port = 8488`, which match the acceptance criterion's curl target.

### Step 3 — Submit a job

After waiting 2 seconds for server startup, submit a single-node graph:
```bash
JOB_ID=$(curl -s -X POST http://127.0.0.1:8488/v1/jobs \
  -H 'Content-Type: application/json' \
  -d '{"graph":{"nodes":[{"id":"n0","type":"PassThrough","inputs":{"value":1}}]},"settings":{}}' \
  | python3 -c "import sys,json;print(json.load(sys.stdin)['job_id'])")
```

The request body contains:
- A single node `n0` of type `PassThrough` with input `{"value": 1}`
- Empty `settings` (no device preference)

The server handler delegates to `JobScheduler::submit()`, which validates the graph (single node, no cycles), persists the job as `Queued`, enqueues it, and returns `202 Accepted` with `{ "job_id": "<uuid>", "queue_position": 1 }`.

### Step 4 — Poll for completion

After waiting 3 seconds (giving the dispatch loop time to process the queued job and the worker to execute it), poll the job status:
```bash
curl -s "http://127.0.0.1:8488/v1/jobs/$JOB_ID" \
  | python3 -c "import sys,json; assert json.load(sys.stdin)['status']=='completed'"
```

The expected flow during this window:
1. The dispatch loop wakes on the submit-triggered notify, selects the idle mock worker, reserves VRAM, transitions the job to `Running`, and sends `WorkerMessage::Execute`.
2. The Python worker receives the execute message, creates a `NodeContext` with `mock=True`, and calls `PassThrough.execute(ctx, value=1)`.
3. The `PassThrough` node returns `{"value": 1}` (both mock and real branches are identical for this trivial node).
4. The worker sends a `Completed` event back to the supervisor.
5. The event loop subscriber transitions the job to `Completed` in the database.
6. The `GET /v1/jobs/:id` handler returns the job with `status: "completed"`.

**Rationale:** The 3-second window is generous — the mock worker executes instantly (no model loading, no torch import). If the assertion fails, the next step surfaces the actual status for diagnosis.

### Step 5 — Kill the background process

```bash
kill %1
```

This terminates the server and triggers graceful shutdown (worker IPC stop, subprocess termination).

### Step 6 — Record terminal output

The literal terminal output from Steps 1–5 is recorded in the implementation report. This includes any server startup logs (hardware detection, worker spawn, Ready events), the HTTP request/response bodies, and the assertion result.

## Public API Surface

None. This task does not introduce or modify any public API items. It exercises existing public APIs:
- `POST /v1/jobs` → `submit_job()` in `crates/anvilml-server/src/handlers/jobs.rs`
- `GET /v1/jobs/:id` → `get_job()` in `crates/anvilml-server/src/handlers/jobs.rs`
- `PassThrough.execute()` in `worker/nodes/passthrough.py`

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| No files created or modified. This task runs the built binary and exercises the full pipeline via HTTP. |

## Tests

This task is a manual Runnable Proof — not an automated test. It has no test file, no test function, and no `#[cfg(test)]` block. The acceptance criterion is a single bash command that builds, runs, submits, polls, asserts, and cleans up.

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| (none — manual proof) | P14-E1 Runnable Proof | A job submitted via `POST /v1/jobs` referencing the real `PassThrough` node is validated, queued, dispatched to a real spawned worker subprocess, executed, and reaches `Completed` | The acceptance bash command (below) exits 0 |

Acceptance command:
```bash
cargo build --release -p anvilml --features mock-hardware && \
./target/release/anvilml & sleep 2 && \
JOB_ID=$(curl -s -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' \
  -d '{"graph":{"nodes":[{"id":"n0","type":"PassThrough","inputs":{"value":1}}]},"settings":{}}' \
  | python3 -c "import sys,json;print(json.load(sys.stdin)['job_id'])") && \
sleep 3 && \
curl -s "http://127.0.0.1:8488/v1/jobs/$JOB_ID" \
  | python3 -c "import sys,json; assert json.load(sys.stdin)['status']=='completed'" && \
kill %1
```

## CI Impact

No CI changes required. This is a manual Runnable Proof — not an automated test that runs in CI. The CI pipeline validates the same pipeline indirectly through unit tests, integration tests, and Python mock/real-mode tests. This proof exercises the full end-to-end chain (HTTP → scheduler → dispatch → worker subprocess → IPC → node execute → status update) as a single coherent trace, which the automated test suite cannot do without a full server lifecycle.

## Platform Considerations

None identified. The acceptance command runs on Linux/WSL2 (the primary development platform). The `mock-hardware` feature ensures no platform-specific GPU detection code is exercised. The Windows cross-check in `ENVIRONMENT.md §7` (targeting `x86_64-pc-windows-gnu`) is sufficient for this task, as it exercises the same Rust code paths that handle the HTTP server and IPC transport on Windows.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The mock Python worker subprocess fails to start (e.g., missing Python 3.12 interpreter, missing venv at `./worker/.venv`, or `ANVILML_WORKER_MOCK=1` not being injected). This would cause the scheduler's `submit()` to return `WorkersUnavailable` (503) because no workers have sent `Ready`. | Medium | High | Ensure the Python venv is provisioned (`bash scripts/install_worker_deps.sh --mode=agent`) before running the proof. Verify the worker is healthy by checking the server's startup logs for "workers spawned" and "worker ready" messages before submitting. |
| The job status remains `Queued` or `Running` after 3 seconds (dispatch loop not waking, worker not responding, or IPC bridge dead). The assertion would fail with `assert json.load(sys.stdin)['status']=='completed'`. | Low | High | Increase the poll sleep to 5 seconds. Add a diagnostic step: if the assertion fails, print the full job JSON to see the actual status and any error field. Check server logs for dispatch loop or bridge errors. |
| Port `8488` is already in use from a prior run. The `TcpListener::bind()` call would panic. | Low | Medium | Add `lsof -ti :8488 | xargs kill 2>/dev/null` before starting the server to ensure the port is free. Or use a different port with `--port` flag. |
| The `PassThrough` node is not registered in `NODE_REGISTRY`, so the scheduler's graph validation rejects it as an unknown node type. | Very Low | High | The node is auto-imported at package load time via `_import_nodes()` in `__init__.py`. If the mock worker fails to import it, the `Ready` event would not include `PassThrough` in its node types, and the scheduler would reject the job with `InvalidGraph`. Verify the `Ready` event includes `PassThrough` by checking the server's startup logs. |

## Acceptance Criteria

- [ ] `cargo build --release -p anvilml --features mock-hardware` exits 0
- [ ] `./target/release/anvilml & sleep 2 && JOB_ID=$(curl -s -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' -d '{"graph":{"nodes":[{"id":"n0","type":"PassThrough","inputs":{"value":1}}]},"settings":{}}' | python3 -c "import sys,json;print(json.load(sys.stdin)['job_id'])") && sleep 3 && curl -s "http://127.0.0.1:8488/v1/jobs/$JOB_ID" | python3 -c "import sys,json; assert json.load(sys.stdin)['status']=='completed'"` exits 0
- [ ] `kill %1` succeeds (background process terminated)
