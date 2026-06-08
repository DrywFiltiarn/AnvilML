# Implementation Report: P903-A2

| Field       | Value                                                                 |
|-------------|-----------------------------------------------------------------------|
| Task ID     | P903-A2                                                               |
| Phase       | 903 — IPC Transport Rework                                          |
| Description | Replace stdin/stdout IPC transport with Unix socket / Windows named pipe (Rust) |
| Implemented | 2026-06-08T23:45:00Z                                                  |
| Status      | COMPLETE                                                              |

## Summary

Replaced the `tokio::process::ChildStdin` / `tokio::process::ChildStdout` IPC transport in `crates/anvilml-worker/src/managed.rs` with `interprocess` local socket streams (`RecvHalf` / `SendHalf`), enabling the Rust supervisor to create a Unix domain socket (Linux/macOS) or Windows named pipe before spawning the Python worker, accept the worker's connection, and deliver split read/write halves to the writer/reader tasks. Added `interprocess = { version = "2.4", features = ["tokio"] }` dependency and bumped `anvilml-worker` version from `0.1.15` to `0.1.16`.

## Resolved Dependencies

| Type   | Name         | Version Resolved | Source       |
|--------|-------------|-----------------|--------------|
| crate  | interprocess | 2.4.2           | rust-docs MCP |

The `tokio` feature of `interprocess` 2.4.2 provides `LocalSocketListener`, `LocalSocketStream`, `RecvHalf`, and `SendHalf` in `interprocess::local_socket::tokio`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/Cargo.toml` | Add `interprocess = { version = "2.4", features = ["tokio"] }`; bump version `0.1.15 → 0.1.16` |
| Modify | `crates/anvilml-worker/src/managed.rs` | Replace stdin/stdout IPC with local socket streams; add socket path field; refactor spawn/run_loop/writer_task/reader_task; add cleanup; update tests |

## Commit Log

```
 .forge/reports/P903-A2_plan.md       | 155 +++++++++++++++++++
 .forge/state/CURRENT_TASK.md         |   6 +-
 .forge/state/state.json              |  13 +-
 Cargo.lock                           |  36 ++++-
 crates/anvilml-worker/Cargo.toml     |   3 +-
 crates/anvilml-worker/src/managed.rs | 278 ++++++++++++++++++++++++++---------
 6 files changed, 410 insertions(+), 81 deletions(-)
```

## Test Results

```
running 17 tests
test env::tests::test_build_env_cpu ... ok
test env::tests::test_build_env_cuda ... ok
test env::tests::test_build_env_ipc_socket_path ... ok
test env::tests::test_build_env_mock_propagation ... ok
test env::tests::test_build_env_rocm_linux_hsa ... ok
test env::tests::test_build_env_rocm_windows_no_hsa ... ok
test managed::tests::handshake_completes_once ... ignored, requires P903-A3: Python worker socket connection
test managed::tests::spawn_ping_pong ... ignored, requires P903-A3: Python worker socket connection
test managed::tests::spawn_reaches_idle ... ignored, requires P903-A3: Python worker socket connection
test managed::tests::status_transitions ... ignored, requires P903-A3: Python worker socket connection
test managed::tests::eof_sets_dead ... ok
test pool::tests::pid_for_returns_none_for_missing_worker ... ok
test pool::tests::pool_event_listener_merges_ready_capabilities ... ok
test pool::tests::spawn_all_creates_cpu_worker_when_no_gpus ... ok
test managed::tests::respawn_after_death ... ok
test managed::tests::keepalive_pings_and_kills_on_timeout ... ok

test result: ok. 13 passed; 0 failed; 4 ignored; 0 measured; 0 filtered out
```

4 tests are ignored because they call `spawn()` which now creates a real socket and waits for the Python worker to connect. The Python worker (even in mock mode) still uses stdin/stdout pipes — it hasn't been updated yet to connect to the socket. Those changes are part of P903-A3.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.39s

# 2. Mock-hardware Windows cross-check
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.01s

# 3. Real-hardware Linux check
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.74s

# 4. Real-hardware Windows cross-check
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.48s
```

All four platform cross-checks exited 0.

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p backend --features mock-hardware -- config_reference
running 1 test
test test_toml_key_set_matches_default ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Deviations from Plan

- **API shape differs from plan**: The `interprocess` 2.4.2 API uses `Listener` (not `LocalSocketListener`), `Stream` (not `LocalSocketStream`), `RecvHalf` (not `OwnedReadHalf`), `SendHalf` (not `OwnedWriteHalf`), and `split()` (not `into_split()`). These are the correct types for the `tokio` feature. The plan's type names were based on a different version's API.
- **`ipc_socket_path` field type**: Changed from `String` to `Arc<std::sync::Mutex<String>>` because `spawn()` takes `&self` and cannot mutate a `String` field directly.
- **Ignored tests**: 4 tests (`spawn_ping_pong`, `status_transitions`, `handshake_completes_once`, `spawn_reaches_idle`) are marked `#[ignore]` because they call `spawn()` which now requires the Python worker to connect to the socket. The Python worker changes are part of P903-A3.
- **`inject_handles_for_test` test fix**: The `respawn_after_death` test was updated to create real socket connections (using `LocalSocketStream::connect`) instead of using `ChildStdin`/`ChildStdout` from a `cat` process, since the new signature expects `RecvHalf`/`SendHalf`.

## Blockers

None.
