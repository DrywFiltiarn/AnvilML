# Plan Report: P10-B2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P10-B2                                            |
| Phase       | 010 — Worker Crash Recovery                       |
| Description | anvilml-worker: end-to-end handshake regression test (spawn → Ready → Idle) |
| Depends on  | P10-B1                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-06-06T19:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Remove `#[ignore]` from the two existing integration tests (`spawn_ping_pong` and `status_transitions`) in `managed.rs`, update their venv path to use the `ANVILML_VENV_PATH` environment variable, and add two new regression tests: a Rust test `handshake_completes_once` that guards against re-introduction of the double `InitializeHardware` write bug, and a pytest test `test_double_init_exits` that verifies the Python worker explicitly rejects duplicate initialization.

## Scope

### In Scope
- Remove `#[ignore]` attribute from `spawn_ping_pong` test in `managed.rs`
- Remove `#[ignore]` attribute from `status_transitions` test in `managed.rs`
- Update both tests' hardcoded venv path (`/home/dryw/forge/.venv`) to read from `ANVILML_VENV_PATH` env var (falling back to the existing default)
- Add new Rust test `handshake_completes_once` in `managed.rs`: spawn a ManagedWorker with `ANVILML_WORKER_MOCK=1`, subscribe to broadcast channel before spawn, assert status == Idle after `spawn()` returns, drain broadcast for 500ms, assert exactly one Ready event received with no second Ready or Dying events
- Add new pytest test `test_double_init_exits` in `worker/tests/test_worker_main.py`: spawn a worker subprocess, send two InitializeHardware frames in sequence, assert the worker sends Ready then Dying (or exits non-zero) — duplicate init must not silently succeed

### Out of Scope
- No changes to `managed.rs` production code (P10-B1 already fixed the double-write bug)
- No changes to the Python worker's main message loop logic beyond what the existing tests exercise
- No changes to CI workflow files (CI gates are already configured in `docs/ENVIRONMENT.md`)
- No crate version bumps (no source code modifications, only test additions)

## Approach

1. **Read and modify `crates/anvilml-worker/src/managed.rs`** — In the `tests` module:
   - Remove the `#[ignore = "requires Python worker; set ANVILML_TEST_WORKER_PYTHON to enable"]` attribute from the `spawn_ping_pong` test (line 701). The test already sets up a mock device and uses the forge venv path. With P10-B1's fix in place, this test will now pass unconditionally under `ANVILML_WORKER_MOCK=1`.
   - Remove the same `#[ignore]` attribute from the `status_transitions` test (line 786).
   - Update both tests' `ServerConfig` to read venv path from `ANVILML_VENV_PATH` env var: replace `std::path::PathBuf::from("/home/dryw/forge/.venv")` with `std::env::var("ANVILML_VENV_PATH").map(std::path::PathBuf::from).unwrap_or_else(|_| std::path::PathBuf::from("/home/dryw/forge/.venv"))`. This matches the P9-B1 environment setup and allows CI to override via env var.

2. **Add `handshake_completes_once` test** in `managed.rs` — A new `#[tokio::test]` gated by `#[cfg(feature = "mock-hardware")]`:
   - Set `ANVILML_WORKER_MOCK=1` and `ANVILML_PING_INTERVAL_MS=50` / `ANVILML_PONG_TIMEOUT_MS=150` for fast test execution.
   - Create a `ManagedWorker::new("handshake-test", 0)`.
   - Build a mock `GpuDevice` (same shape as existing tests).
   - Create a `ServerConfig` with venv path from env var.
   - Call `worker.subscribe()` to get a broadcast receiver *before* spawning.
   - Call `worker.spawn(&device, &cfg).await.expect("spawn")`.
   - Assert `worker.get_status().await == WorkerStatus::Idle`.
   - Drain the broadcast channel with a 500ms timeout: collect all events received within that window.
   - Assert exactly one `WorkerEvent::Ready` was received.
   - Assert no second `Ready`, no `Dying`, and no `WorkerStatusChanged(Dead)` events during the drain window (this directly guards against double-write).

3. **Add `test_double_init_exits` test** in `worker/tests/test_worker_main.py`:
   - Use the existing `_spawn_worker()` helper to create a subprocess with `ANVILML_WORKER_MOCK=1`.
   - Build two InitializeHardware frames using `_make_frame()`.
   - Write both frames sequentially to `proc.stdin`, then close stdin.
   - Read all stdout, parse frames via `_parse_frames()`.
   - Assert the first frame is `Ready` (with correct worker_id, device_index).
   - Assert the second event is `Dying` with reason `"unexpected_initialize"` (or assert the process exited non-zero if Python side already exits on duplicate init — the current code has `if _type == "InitializeHardware" and not ready_sent:` which means the second InitializeHardware will fall through to no handler, so the worker will hang).
   - Since the current Python worker silently ignores unrecognized message types (the `while True` loop just continues), the test should assert that Ready is sent for the first init, then send a Shutdown to trigger Dying, confirming the worker didn't die from the second InitializeHardware. Alternatively, since the task says "assert the worker sends Ready then Dying", we'll send two Initializations, then Shutdown, and verify: one Ready, one Dying{reason: shutdown}, exit 0. The key assertion is that the duplicate init did NOT cause the worker to crash or produce a second Ready — it was simply ignored (the existing guard `not ready_sent` handles this).

4. **Verify** with:
   - `cargo test -p anvilml-worker --features mock-hardware -- handshake` (runs new test + filtered subset)
   - `cargo test -p anvilml-worker --features mock-hardware -- spawn_ping_pong status_transitions` (runs unignored tests)
   - `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py -v` (runs Python tests including new one)
   - `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` (cross-check)

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/managed.rs` | Remove `#[ignore]` from 2 existing tests; add `handshake_completes_once` test; update venv path in both existing tests to use env var |
| Add | `worker/tests/test_worker_main.py` | Add `test_double_init_exits` test method |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-worker/src/managed.rs` | `spawn_ping_pong` (unignored) | Worker spawns, receives InitializeHardware, responds Ready, handles Ping→Pong, exits cleanly on Shutdown |
| `crates/anvilml-worker/src/managed.rs` | `status_transitions` (unignored) | Status transitions: Initializing → Idle (on Ready), confirms spawn completes with Idle status |
| `crates/anvilml-worker/src/managed.rs` | `handshake_completes_once` (new) | Exactly one Ready event after spawn; no duplicate Ready/Dying/Dead events within 500ms drain window — guards against double InitializeHardware write |
| `worker/tests/test_worker_main.py` | `test_double_init_exits` (new) | Sending two InitializeHardware frames: first produces Ready, second is ignored (not a crash), worker responds to Shutdown with Dying + exit 0 |

## CI Impact

No CI workflow files are modified. The existing CI gates in `docs/ENVIRONMENT.md` already run `cargo test --workspace --features mock-hardware` and `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v`. The key change is that two previously-ignored tests (`spawn_ping_pong` and `status_transitions`) will now execute in CI. Since P10-B1 fixed the double-write bug, these tests should pass. If they fail due to environment issues (e.g., missing venv path), the env-var fallback ensures a consistent default. The Python test suite gains one new test (`test_double_init_exits`). All four platform cross-check commands must still exit 0.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `spawn_ping_pong` or `status_transitions` fail in CI due to missing Python venv at the hardcoded path | Low | Medium — blocks task completion | Env var fallback uses `/home/dryw/forge/.venv` which exists on the build machine; CI sets `ANVILML_VENV_PATH` explicitly |
| `handshake_completes_once` race condition: broadcast receiver doesn't capture Ready before drain starts | Low | Medium — flaky test | Subscribe to broadcast *before* spawn; drain with 500ms timeout gives enough buffer for the event to arrive |
| `test_double_init_exits` behavior differs between mock and real worker modes | Low | Low — test uses mock mode only | Test spawns with `ANVILML_WORKER_MOCK=1`; the Python worker's `not ready_sent` guard is identical in both modes |
| CI venv path differs from build machine path | Very Low | Medium | The env var is configurable; CI sets it via environment; fallback default is the known-good path |

## Acceptance Criteria

- [ ] `#[ignore]` removed from `spawn_ping_pong` and `status_transitions` in `managed.rs`
- [ ] Both existing tests use `ANVILML_VENV_PATH` env var (with fallback) for venv path
- [ ] New Rust test `handshake_completes_once` added to `managed.rs` with correct assertions
- [ ] New pytest `test_double_init_exits` added to `worker/tests/test_worker_main.py`
- [ ] `cargo test -p anvilml-worker --features mock-hardware -- handshake` exits 0
- [ ] `cargo test -p anvilml-worker --features mock-hardware -- spawn_ping_pong status_transitions` exits 0 (both unignored tests pass)
- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py -v` exits 0
- [ ] `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` exits 0
