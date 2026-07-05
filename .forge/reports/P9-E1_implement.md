# Implementation Report: P9-E1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P9-E1                           |
| Phase         | 009 — Real Worker Startup       |
| Description   | anvilml-worker: integration test, real subprocess sends Ready |
| Implemented   | 2026-07-05T19:22:00Z            |
| Status        | COMPLETE                        |

## Summary

Created `crates/anvilml-worker/tests/real_startup_tests.rs` — an integration test that spawns a genuine `worker_main.py` subprocess targeting CPU in real mode (no `ANVILML_WORKER_MOCK` flag), connects a ZeroMQ DEALER socket to the worker's ROUTER transport, and verifies that the worker sends a `Ready` event with `capabilities_source = "pytorch"` and empty `node_types` within 10 seconds. This is the phase's Runnable Proof.

During implementation, two prerequisite issues were discovered and fixed:
1. The `RouterTransport::recv()` method only accepted 3-frame ROUTER messages, but pyzmq sends 1-frame messages which the ROUTER delivers as 2 frames. Fixed by accepting both 2-frame and 3-frame messages.
2. The Python worker's `Ready` event only included `_type`, `capabilities_source`, and `node_types`, but the Rust `WorkerEvent::Ready` struct requires 13 fields. Fixed by updating both `_real_startup_sequence()` and `_mock_startup_sequence()` in `worker_main.py` to include all required fields.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| crate  | zeromq    | 0.6.0            | rust-docs MCP  |

No new dependencies were added. All types used (`RouterTransport`, `WorkerEvent`, `spawn_worker`, `build_command`, `WorkerEnv`, `DeviceType`, `DealerSocket`, `SocketOptions`, `PeerIdentity`, `Bytes`) are from existing dependencies.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-worker/tests/real_startup_tests.rs` | New integration test file with `test_real_subprocess_sends_ready` |
| MODIFY | `crates/anvilml-worker/Cargo.toml` | Bump version 0.1.25 → 0.1.26 |
| MODIFY | `crates/anvilml-ipc/Cargo.toml` | Bump version 0.1.11 → 0.1.12 |
| MODIFY | `crates/anvilml-ipc/src/transport.rs` | Accept 2-frame and 3-frame ROUTER messages for pyzmq compatibility |
| MODIFY | `crates/anvilml-ipc/tests/roundtrip_tests.rs` | Update `test_recv_malformed_frames_returns_error` to test deserialization failure instead of frame count |
| MODIFY | `worker/worker_main.py` | Include all 13 `WorkerEvent::Ready` fields in both real and mock startup sequences |
| MODIFY | `docs/TESTS.md` | Add entry for `test_real_subprocess_sends_ready` |

## Commit Log

```
 .forge/reports/P9-E1_plan.md                      | 163 ++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                      |   6 +-
 .forge/state/state.json                           |  13 +-
 Cargo.lock                                        |   4 +-
 crates/anvilml-ipc/Cargo.toml                     |   2 +-
 crates/anvilml-ipc/src/transport.rs               |  30 ++--
 crates/anvilml-ipc/tests/roundtrip_tests.rs       |  18 +--
 crates/anvilml-worker/Cargo.toml                  |   2 +-
 crates/anvilml-worker/tests/real_startup_tests.rs | 176 ++++++++++++++++++++++
 docs/TESTS.md                                     |  12 ++
 worker/worker_main.py                             |  54 +++++++
 11 files changed, 448 insertions(+), 32 deletions(-)
```

## Test Results

```
     Running tests/real_startup_tests.rs (target/debug/deps/real_startup_tests-7956b8e2c8637b57)

running 1 test
test test_real_subprocess_sends_ready ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.96s
```

Full workspace test suite: all 260+ tests passed across all crates. No failures, no ignored tests.

## Format Gate

```
cargo fmt --all -- --check
```
Exits 0 — no formatting drift.

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 31.75s

# 3. Real-hardware Linux
cargo check --bin anvilml
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.34s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.47s
```

All four platform cross-checks exited 0.

## Project Gates

**Gate 1 — Config Surface Sync:**
```
cargo test -p anvilml --features mock-hardware -- config_reference
test tests::config_reference_matches_defaults ... ok
```
Exits 0.

**Gate 2 — OpenAPI Drift:**
`api/openapi.json` does not yet exist in this repo — gate is N/A.

## Public API Delta

```
git diff HEAD -- crates/anvilml-worker/tests/real_startup_tests.rs crates/anvilml-ipc/src/transport.rs crates/anvilml-ipc/tests/roundtrip_tests.rs crates/anvilml-worker/Cargo.toml crates/anvilml-ipc/Cargo.toml worker/worker_main.py docs/TESTS.md | grep "^+.*pub " | head -40
```
No new `pub` items introduced. The test file contains only a `#[tokio::test]` function (not `pub`). The `RouterTransport::recv()` change modifies existing behavior but does not change the public API surface.

## Deviations from Plan

1. **Path resolution:** The plan specified `spawn_worker(Path::new("worker/.venv"), env)` with a relative path. Since `cargo test` runs from `target/debug/deps`, relative paths don't resolve. Fixed by resolving the repo root from `env!("CARGO_MANIFEST_DIR")` and using `build_command()` with `.current_dir()` and `PYTHONPATH` set to the repo root.

2. **ROUTER frame count:** The plan assumed `RouterTransport::recv()` receives 3-frame messages. In practice, pyzmq sends 1-frame messages which the ROUTER delivers as 2 frames (`[identity, payload]`). Fixed `RouterTransport::recv()` to accept both 2-frame and 3-frame messages, with the payload always being the last frame.

3. **Python worker Ready event fields:** The plan assumed the worker sends a `Ready` event with `capabilities_source` and `node_types`. In practice, the Python worker only sent these two fields, but the Rust `WorkerEvent::Ready` struct requires 13 fields. Fixed by updating both `_real_startup_sequence()` and `_mock_startup_sequence()` in `worker_main.py` to include all required fields.

4. **Test uses `build_command` instead of `spawn_worker`:** Due to the path resolution issue, the test constructs the command via `build_command()` and manually sets `.current_dir()` and `.env("PYTHONPATH", ...)`, rather than calling `spawn_worker()`. The `spawn_worker()` function is not used because it doesn't expose CWD or extra environment variables.

## Blockers

None.
