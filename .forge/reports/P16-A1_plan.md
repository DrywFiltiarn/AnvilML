# Plan Report: P16-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P16-A1                                      |
| Phase       | 016 — Job Cancellation                        |
| Description | worker: cooperative cancel — check cancel_flag between nodes |
| Depends on  | P15-A3                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-06-10T10:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Add cooperative cancellation support to the Python worker (`worker/worker_main.py`). When the worker receives a `CancelJob{job_id}` IPC message during an in-flight `Execute`, it sets an internal per-job cancel flag. The mock `Execute` loop checks this flag before executing each node; if set, it emits `Cancelled{job_id}` (and **no** `Completed`) and stops immediately. Additionally, add support for the `ANVILML_MOCK_NODE_DELAY_MS` environment variable so the mock executor sleeps between nodes, making cancellation observable in integration tests (P16-A4).

## Scope

### In Scope
- Modify `worker/worker_main.py` only (one file).
- Add a `cancel_flag: dict[str, bool]` (or equivalent) in the worker's main loop to track per-job cancellation state.
- Handle `CancelJob` IPC message in the main message loop: set `cancel_flag[job_id] = True`.
- Pass `cancel_flag` into `_execute_mock` and check it before each node iteration.
- If `cancel_flag[job_id]` is set at a checkpoint, emit `Cancelled{job_id}` instead of `Completed` and return early.
- Read `ANVILML_MOCK_NODE_DELAY_MS` env var in `_execute_mock`; if set, `time.sleep(delay_secs)` between nodes.
- Update the module docstring to document the new `CancelJob` message and `Cancelled` event.

### Out of Scope
- Any Rust-side changes (scheduler, server, IPC layer) — those are P16-A2 and P16-A3.
- Any integration tests — those are P16-A4.
- Real (non-mock) model execution cancel support — deferred to a later phase.
- Sampler-level per-step cancellation callbacks — deferred to a later phase.
- Version bumping any Rust crate (no Rust files modified).

## Approach

1. **Add cancel-flag tracking to the worker's main loop.**
   - Declare `cancel_flag: dict[str, bool] = {}` at module level (or as a closure variable in `main()`), keyed by job ID string.
   - In the main message loop `while True`, add a new `if _type == "CancelJob"` branch before the existing `Execute` handler.
   - When `CancelJob` is received, extract `job_id` from the message and set `cancel_flag[job_id] = True`.
   - Continue the loop (the flag will be checked on the next node iteration of the currently executing job).

2. **Modify `_execute_mock` to accept and check a cancel flag.**
   - Change the signature: `_execute_mock(job_id, graph, settings, device_index, cancel_flag)` — `cancel_flag` is a `dict[str, bool]`.
   - At the top of the node loop, before processing each node, check `if cancel_flag.get(job_id):`.
   - If cancelled: emit `Cancelled{job_id}` via `ipc.write_frame`, then `return` (no `Completed` emitted).
   - After the loop completes normally (all nodes processed and no cancellation), emit `Completed` as before.

3. **Add `ANVILML_MOCK_NODE_DELAY_MS` support.**
   - At the top of `_execute_mock`, read `delay_ms = int(os.environ.get("ANVILML_MOCK_NODE_DELAY_MS", "0"))`.
   - After emitting `Progress` for each node (and before the next iteration), if `delay_ms > 0`: `time.sleep(delay_ms / 1000.0)`.
   - This delay is applied per-node so that a cancel arriving mid-execution will be observable.

4. **Wire cancel_flag into Execute call.**
   - In the `Execute` handler in `main()`, pass the `cancel_flag` dict to `_execute_mock`:
     `_execute_mock(job_id, graph, settings, device_index, cancel_flag)`.
   - After `_execute_mock` returns (whether via `Completed` or `Cancelled`), clean up the flag: `cancel_flag.pop(job_id, None)`.

5. **Update module docstring.**
   - Add `CancelJob{job_id}` to the Rust→Python message list.
   - Add `Cancelled{job_id}` to the Python→Rust event list.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/worker_main.py` | Add `CancelJob` handling, cancel flag, `_execute_mock` signature change, `ANVILML_MOCK_NODE_DELAY_MS` support, docstring update |

No Rust files are modified. No version bumps needed.

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `worker/tests/test_worker_main.py` | `test_cancel_job_during_execute` (new) | Execute a 3-node job, send `CancelJob` after first `Progress` frame arrives → verify `Cancelled` is emitted (no `Completed`), `Cancelled` has correct `job_id` |
| `worker/tests/test_worker_main.py` | `test_cancel_before_execute` (new) | Send `CancelJob` before `Execute` → worker processes `CancelJob` (flag set), then receives `Execute` → should emit `Cancelled` before any `Progress` |
| `worker/tests/test_worker_main.py` | `test_mock_node_delay_ms` (new) | Set `ANVILML_MOCK_NODE_DELAY_MS=50`, execute a 3-node job → verify total elapsed time ≥ 100ms (2 inter-node delays × 50ms) |
| `worker/tests/test_worker_main.py` | `test_execute_progress_completed` (existing) | Must still pass — no regression in normal execution |
| `worker/tests/test_worker_main.py` | `test_execute_saveimage_imageready` (existing) | Must still pass — no regression in SaveImage path |

## CI Impact

The Python worker test command (`ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v`) will run the new tests. No CI workflow files are modified. No new CI gates are needed. The Rust test suite is unaffected since no Rust code changes. The `mock-hardware` feature flag is unchanged.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `CancelJob` arrives while worker is idle (no Execute in progress) | Medium | Low | Flag is set but never checked; harmless no-op. Worker continues idle, waiting for next message. |
| `CancelJob` arrives for a job_id that was already completed | Low | Low | Flag is cleaned up after `_execute_mock` returns; if a second `CancelJob` arrives for same job_id, flag is set but no Execute will read it. Harmless. |
| `ANVILML_MOCK_NODE_DELAY_MS` not set → `int("0")` works correctly | Low | Low | Default value `"0"` ensures `delay_ms == 0` and no sleep is introduced. |
| Concurrent `Execute` calls (not expected in current architecture) | Low | Low | Single-threaded worker loop; only one `Execute` runs at a time. No race condition. |
| Existing tests break due to signature change of `_execute_mock` | Low | Medium | `_execute_mock` is not imported by any other module (it is private, prefixed with `_`). Only `worker_main.py` calls it. |

## Acceptance Criteria

- [ ] `worker/worker_main.py` handles `CancelJob{job_id}` IPC message by setting a per-job cancel flag
- [ ] `_execute_mock` checks the cancel flag before each node; if set, emits `Cancelled{job_id}` and returns without `Completed`
- [ ] `ANVILML_MOCK_NODE_DELAY_MS` env var causes per-node sleep in `_execute_mock`
- [ ] Module docstring documents `CancelJob` and `Cancelled` in the message protocol
- [ ] New pytest tests (`test_cancel_job_during_execute`, `test_cancel_before_execute`, `test_mock_node_delay_ms`) pass under `ANVILML_WORKER_MOCK=1`
- [ ] All existing worker tests still pass (no regression)
- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v` exits 0
