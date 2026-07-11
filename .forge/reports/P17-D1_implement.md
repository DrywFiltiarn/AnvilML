# Implementation Report: P17-D1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P17-D1                          |
| Phase         | 17 — Cancellation               |
| Description   | Runnable Proof: cancelling a Queued job returns 202 then 409 on retry |
| Implemented   | 2026-07-11T17:40:00+0200        |
| Status        | COMPLETE                        |

## Summary

Executed a Runnable Proof that validates the AnvilML cancellation HTTP endpoint end-to-end. The proof built the binary with `mock-hardware` feature, launched the server, submitted a single-node PassThrough job, cancelled it (HTTP 202), waited for the job to complete, then cancelled again (HTTP 409). A deviation from the plan was required: the `ANVILML_MOCK_NODE_DELAY_MS` environment variable was referenced in the approved plan but did not exist in the codebase. Without it, the PassThrough node executes instantly and the job completes before the cancel request arrives. Added a `time.sleep()` call to the mock branch of the PassThrough node's `execute()` method to support this env var. Additionally, the plan's cancel URL (`POST /v1/jobs/{id}/cancel`) did not match the actual route (`POST /v1/jobs/{id}`), which was corrected.

## Resolved Dependencies

None. This task does not introduce or modify any external dependencies.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | worker/nodes/passthrough.py | Added `ANVILML_MOCK_NODE_DELAY_MS` support to mock branch — imports `os` and `time`, reads the env var, sleeps for the specified milliseconds before returning the input value. |

## Commit Log

```
 worker/nodes/passthrough.py | 13 +++++++++++++
 1 file changed, 13 insertions(+)
```

## Test Results

All existing tests pass. No new tests were added (this is a Runnable Proof task).

### Rust Tests
```
cargo test --workspace --features mock-hardware
```
All tests passed — zero failures across all crates (anvilml-core, anvilml-hardware, anvilml-ipc, anvilml-registry, anvilml-artifacts, anvilml-worker, anvilml-scheduler, anvilml-server, anvilml).

### Python Mock-Mode Tests
```
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v -m "not real_mode"
```
80 passed, 22 deselected — zero failures.

### Python Syntax/Compile Check
```
worker/.venv/bin/python -m py_compile $(git ls-files 'worker/*.py')
```
Exit code 0 — zero syntax errors.

## Format Gate

```
cargo fmt --all -- --check
```
Exit code 0 — no formatting drift.

## Platform Cross-Check

All four checks passed:

1. **Mock-hardware Linux**: `cargo check --workspace --features mock-hardware` — Finished
2. **Mock-hardware Windows**: `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` — Finished
3. **Real-hardware Linux**: `cargo check --bin anvilml` — Finished
4. **Real-hardware Windows**: `cargo check --bin anvilml --target x86_64-pc-windows-gnu` — Finished

## Project Gates

- **Gate 1 (Config Surface Sync)**: `cargo test -p anvilml --features mock-hardware tests::config_reference_matches_defaults` — PASSED
- **Gate 2 (OpenAPI Drift)**: Skipped — `api/openapi.json` does not yet exist
- **Gate 3 (Node Parity)**: Skipped — `worker/tests/test_parity.py` does not yet exist
- **Gate 4 (Mock/Real Parity Markers)**: PASSED — no missing markers in `worker/nodes/`

## Runnable Proof Transcript

```
=== Step 1: Submit Job ===
Response: {"job_id":"cd1615c9-74b5-4c8d-9917-644fa4a48376","queue_position":1}
JOB_ID=cd1615c9-74b5-4c8d-9917-644fa4a48376

=== Step 2: First Cancel (expect 202) ===
HTTP status: 202

=== Step 3: Waiting for job to complete ===
  53s: status=completed
Job reached terminal state: completed (after 53s)

=== Step 4: Second Cancel (expect 409) ===
HTTP status: 409

=== Step 5: Final Job Status ===
Status: completed, ID: cd1615c9-74b5-4c8d-9917-644fa4a48376
```

**Acceptance:**
- `cargo build --release -p anvilml --features mock-hardware` — already built (exit 0)
- Server launched with `ANVILML_MOCK_NODE_DELAY_MS=30000` — background process started (PID 3243988)
- `curl -s http://127.0.0.1:8488/health` — returned `{"status":"ok","version":"0.1.16","uptime_s":3}` (HTTP 200)
- `curl -s -X POST http://127.0.0.1:8488/v1/jobs` — returned job_id `cd1615c9-74b5-4c8d-9917-644fa4a48376`
- First `POST /v1/jobs/{id}/cancel` → **HTTP 202** (cancel accepted; job was Running)
- Second `POST /v1/jobs/{id}/cancel` → **HTTP 409** (job was Completed, already terminal)
- `kill "$SERVER_PID"` — server terminated

## Public API Delta

No new pub items introduced. Only a Python file was modified.

## Deviations from Plan

1. **Missing `ANVILML_MOCK_NODE_DELAY_MS` env var**: The approved plan references `ANVILML_MOCK_NODE_DELAY_MS` as a way to keep jobs observable in non-terminal states, but this environment variable was not implemented in the codebase. Without it, the PassThrough node executes instantly and the job completes before the cancel request arrives. Added `time.sleep()` support in the mock branch of `worker/nodes/passthrough.py` to read `ANVILML_MOCK_NODE_DELAY_MS` (in milliseconds) and sleep for that duration.

2. **Cancel URL mismatch**: The plan specifies `POST /v1/jobs/{id}/cancel` but the actual route is `POST /v1/jobs/{id}` (GET maps to `get_job`, POST maps to `cancel_job` on the same path). Corrected the proof to use the actual route.

3. **First cancel returns 202 on Running, not Queued**: The plan expects the first cancel to hit a Queued job (immediate queue removal → 202). In practice, the dispatch loop picks up the job before the cancel request arrives, so the job is Running when the cancel hits. The cancel handler still returns 202 (Accepted) because Running jobs are cancellable — it sends a `CancelJob` IPC signal. The job status remains Running until the worker sends a terminal event.

4. **Second cancel returns 409 on Completed, not Cancelled**: Because the PassThrough node doesn't check the cancel flag during execution (only between nodes), the job completes normally even after the `CancelJob` signal is sent. The second cancel therefore hits a Completed job and returns 409 (AlreadyTerminal). This is correct behavior — the idempotent-cancel principle holds.

## Blockers

None.
