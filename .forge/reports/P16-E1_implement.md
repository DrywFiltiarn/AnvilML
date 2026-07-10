# Implementation Report: P16-E1

| Field         | Value                                                     |
|---------------|-----------------------------------------------------------|
| Task ID       | P16-E1                                                    |
| Phase         | 16 — Live Events                                          |
| Description   | Runnable Proof: WebSocket client observes JobCompleted for PassThrough job |
| Implemented   | 2026-07-10T21:50:00Z                                      |
| Status        | COMPLETE                                                  |

## Summary

Created `scripts/run_proof_p16_e1.py`, a self-contained Python script that connects to
the live event stream at `ws://127.0.0.1:8488/v1/events`, consumes the initial
`SystemStats` frame, submits a single-node PassThrough job via `POST /v1/jobs`, and
asserts that a `job_completed` JSON frame with the matching `job_id` arrives on the
WebSocket within 10 seconds. The proof script was executed end-to-end against the
mock-hardware binary and passed successfully.

During inspection, a pre-existing bug was discovered and fixed in
`crates/anvilml-scheduler/src/event_loop.rs`: the event loop's catch-all match arm
called `map_worker_event()` on all non-terminal events, but `map_worker_event()` panics
on `Ready`, `Pong`, `Dying`, and `MemoryReport` variants. Because the `Demux` fans out
ALL events to all subscribers, `Ready` events from worker startup reached the event loop
and caused a panic, terminating the event loop task and closing the broadcast channel —
so no WebSocket events were ever delivered to clients. The fix replaces the catch-all
with an explicit `Progress` arm (which constructs the `WsEvent::JobProgress` directly)
and an explicit skip arm for `Ready`/`Pong`/`Dying`/`MemoryReport` events.

## Resolved Dependencies

| Type   | Name       | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| python | websockets| 16.0 (installed) | venv already provisioned |

Note: `pypi-query` MCP reported 16.1 as latest; the project's venv carries 16.0. Both
use the same async context manager API (`async with websockets.connect(...) as ws:`).
The script uses `urllib.request` from the Python stdlib for the HTTP POST — no additional
Python dependencies.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `scripts/run_proof_p16_e1.py` | Runnable proof script: WebSocket client observes JobCompleted for PassThrough job |
| MODIFY | `crates/anvilml-scheduler/src/event_loop.rs` | Fix: filter Ready/Pong/Dying/MemoryReport events before broadcast (catch-all bug) |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version 0.1.24 → 0.1.25 |
| MODIFY | `docs/TESTS.md` | Add entry for `proof_ws_job_completed_pass_through` test |

## Commit Log

```
 .forge/state/CURRENT_TASK.md               |  6 +++---
 .forge/state/state.json                    | 16 +++++++--------
 Cargo.lock                                 |  2 +-
 crates/anvilml-scheduler/Cargo.toml        |  2 +-
 crates/anvilml-scheduler/src/event_loop.rs | 33 ++++++++++++++++++++++++++----
 docs/TESTS.md                              | 12 +++++++++++
 6 files changed, 54 insertions(+), 17 deletions(-)
```

## Test Results

```
     Running tests/event_loop_tests.rs (target/debug/deps/event_loop_tests-d397d0e518b20308)

running 23 tests
test test_image_ready_publishes_after_save ... ok
test test_map_cancelled ... ok
test test_map_completed ... ok
test test_map_progress ... ok
test test_map_failed ... ok
test test_image_ready_malformed_base64_errors ... ok
test test_image_ready_artifact_meta_fields_match ... ok
test test_image_ready_saves_artifact ... ok
test test_progress_still_published_via_map_worker_event ... ok
test test_spawn_event_loop_handles_recv_error ... ok
test test_image_ready_empty_image_b64 ... ok
test test_failed_restores_worker_idle_wakes_dispatch ... ok
test test_cancelled_persists_status_and_releases_ledger ... ok
test test_failed_persists_status_error_and_releases_ledger ... ok
test test_completed_persists_status_and_releases_ledger ... ok
test test_progress_does_not_wake_dispatch ... ok
test test_cancelled_restores_worker_idle_wakes_dispatch ... ok
test test_completed_restores_worker_idle_wakes_dispatch ... ok
test test_terminal_event_unknown_job_logs_warning ... ok
test test_queued_job_dispatched_after_first_completes ... ok
test test_spawn_event_loop_subscription_exists_before_return ... ok
test test_spawn_event_loop_receives_and_publishes ... ok
test test_terminal_events_publish_ws_event ... ok

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
```

Full workspace test suite: 327 tests passed, 0 failed, 0 ignored (all crates).
Python mock-mode: 55 passed, 22 deselected.
Python real-mode: 22 passed, 55 deselected.

## Runnable Proof Transcript

```
=== Running proof ===
{
  "type": "job_completed",
  "job_id": "f8a7a780-0c8c-4fa8-b6f9-9c89cb855e2a",
  "elapsed_ms": 0
}

Proof passed: job_completed for f8a7a780-0c8c-4fa8-b6f9-9c89cb855e2a received.
EXIT=0
```

The proof script connected to `ws://127.0.0.1:8488/v1/events`, consumed the initial
`SystemStats` frame, submitted a PassThrough job via `POST /v1/jobs`, and received a
`job_completed` frame with the matching `job_id` within 10 seconds. Exit code: 0.

## Format Gate

```
EXIT=0
```

`cargo fmt --all -- --check` exited 0 — no formatting drift.

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.80s

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.58s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.22s

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.62s
```

All four platform cross-checks exit 0.

## Project Gates

**Gate 1 — Config Surface Sync:** `cargo test -p anvilml --features mock-hardware -- config_reference`
exits 0 — `config_reference_matches_defaults` passes.

**Gate 2 — OpenAPI Drift:** `api/openapi.json` does not yet exist (the `anvilml-openapi`
binary prints "openapi generation stub"). Per the gate definition, this gate is skipped
when the file does not exist.

## Public API Delta

```
(no output — no new pub items)
```

No new `pub` items were introduced. The event loop fix modifies internal match arms
but does not change any public function signatures or types.

## Deviations from Plan

- **Pre-existing bug fix in `crates/anvilml-scheduler/src/event_loop.rs`:** The plan's
  "Files Affected" table listed only `scripts/run_proof_p16_e1.py` as CREATE. However,
  during inspection the proof failed because the event loop panicked on `Ready` events.
  The `Demux` fans out ALL events to all subscribers, including `Ready` events that are
  supposed to be handled only by the node registry. The catch-all match arm called
  `map_worker_event()` which panics on `Ready`/`Pong`/`Dying`/`MemoryReport`. This bug
  was fixed by replacing the catch-all with an explicit `Progress` arm and an explicit
  skip arm for non-broadcast events. This is a necessary fix for the proof to work
  and was discovered during codebase inspection (§9.4: pre-existing errors in files this
  task modifies must be fixed, not skipped).

- **Version bump:** `crates/anvilml-scheduler` was bumped from 0.1.24 to 0.1.25 because
  its source file was modified (the bug fix).

## Blockers

None.
