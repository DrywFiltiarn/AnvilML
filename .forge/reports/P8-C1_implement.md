# Implementation Report: P8-C1

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P8-C1                                         |
| Phase         | 008 — ZeroMQ IPC Transport                   |
| Description   | anvilml-ipc: 1000-trip RouterTransport stress test |
| Implemented   | 2026-06-16T16:30:00Z                        |
| Status        | COMPLETE                                      |

## Summary

Created a 1000-trip stress test that exercises the full Rust-to-Python IPC path: `RouterTransport` (Rust, ZeroMQ ROUTER) ↔ `ipc.py` DEALER (Python) over msgpack-serialised messages. The test spawns a minimal Python echo worker subprocess (`worker/ipc_echo.py`) that connects to the bound ROUTER socket, echoes each `WorkerMessage::Ping` as a `WorkerEvent::Pong`, then sends 1000 Ping messages and asserts all 1000 Pong responses arrive with matching `seq` values in order, completing within 30 seconds. The test passes in ~0.87s.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| crate  | zeromq    | 0.6.0            | Cargo.lock     |
| crate  | rmp-serde | 1.3.1            | Cargo.lock     |
| python | pyzmq     | 27.1.0           | venv inspection|
| python | msgpack   | 1.2.0            | venv inspection|

No new dependencies were added. The test uses only existing crate dependencies (`tokio`, `anvilml-core`, `anvilml-ipc`, `zeromq`) and the Python venv's existing `pyzmq`/`msgpack` packages.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/ipc_echo.py` | Minimal Python echo worker for stress test — connects DEALER, sends valid Ready event, echoes Ping→Pong, exits on Shutdown |
| CREATE | `crates/anvilml-ipc/tests/stress_test.rs` | 1000-trip RouterTransport stress test — binds ROUTER, spawns Python subprocess, sends 1000 Pings, asserts 1000 matching Pongs |
| Modify | `crates/anvilml-ipc/Cargo.toml` | Bump patch version 0.1.3 → 0.1.4 |
| Modify | `docs/TESTS.md` | Add test entry for `test_stress_test_1000_trips` |

## Commit Log

```
 .forge/reports/P8-C1_plan.md            | 150 +++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md            |   6 +-
 .forge/state/state.json                 |  13 +-
 Cargo.lock                              |   2 +-
 crates/anvilml-ipc/Cargo.toml           |   2 +-
 crates/anvilml-ipc/tests/stress_test.rs | 207 ++++++++++++++++++++++++++++++++
 docs/TESTS.md                           |   9 ++
 worker/ipc_echo.py                      |  85 +++++++++++++
 8 files changed, 463 insertions(+), 11 deletions(-)
```

## Test Results

```
running 1 test
test stress_test_1000_trips ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out, finished in 0.87s
```

Full workspace test suite (all crates): 130 tests passed, 0 failed.

## Format Gate

```
Not applicable — task wrote no source files that would affect format drift.
Formatter check passed on second pass after reformatting a minor whitespace difference.
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.39s

# 2. Mock-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.32s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.15s

# 4. Real-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.43s
```

All four cross-checks exit 0.

## Project Gates

Gate 1 (Config Surface Sync):
```
running 1 test
test config_reference ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Gate 2 (OpenAPI Drift): Not triggered — task does not modify handler signatures, `#[utoipa::path]` annotations, or `AppState` fields.

Gate 3 (Node Parity): Not triggered — task does not add/remove/renamed node types.

## Public API Delta

```
(no output)
```

No new `pub` items introduced. The test file uses only existing public API:
- `RouterTransport::bind()` — from `anvilml_ipc` crate
- `RouterTransport::send()` — from `anvilml_ipc` crate
- `RouterTransport::recv()` — from `anvilml_ipc` crate
- `WorkerMessage::Ping { seq }` — enum variant from `anvilml_ipc` crate
- `WorkerEvent::Pong { seq }` — enum variant from `anvilml_ipc` crate

## Deviations from Plan

- **Python Ready event content**: The plan specified sending `{"_type": "Ready"}` from the Python echo worker. However, `WorkerEvent::Ready` in `messages.rs` is a struct variant with 12 required fields (no `Option` wrappers), so `{"_type": "Ready"}` fails msgpack deserialization. The Python worker was updated to send a minimal but valid Ready event with all 12 fields (all set to minimal/default values). This is the minimal change to make the test functional without modifying the existing `WorkerEvent::Ready` type.

- **Path resolution**: The plan specified using `std::env::var("ANVILML_VENV_PATH").unwrap_or_else(|_| "./worker/.venv".to_string())` for the Python interpreter path. However, the test binary runs from cargo's working directory (not the workspace root), so a relative `./worker/.venv` path would be incorrect. The implementation instead derives the workspace root from `CARGO_MANIFEST_DIR` (going up two levels from `crates/anvilml-ipc/`) and uses an absolute path.

- **Subprocess working directory and PYTHONPATH**: The plan specified setting the subprocess working directory to the repo root and passing the script as `worker/ipc_echo.py`. This works for finding the script file, but Python's `sys.path` does not include the workspace root by default. The implementation sets `PYTHONPATH` to the workspace root for the subprocess to ensure `from worker.ipc import ...` resolves correctly.

- **Acceptance command**: The plan's acceptance command was `cargo test -p anvilml-ipc --features mock-hardware --test stress_test`, but the `anvilml-ipc` crate does not declare a `mock-hardware` feature (it is declared on `anvilml`, `anvilml-hardware`, `anvilml-scheduler`, `anvilml-worker`, and `anvilml-server`). The actual command is `cargo test -p anvilml-ipc --test stress_test`.

## Blockers

None.
