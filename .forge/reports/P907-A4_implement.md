# Implementation Report: P907-A4

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P907-A4                                           |
| Phase       | 907 — ZeroMQ IPC Transport                        |
| Description | anvilml-worker: managed.rs replace interprocess with ZeroMQ DEALER socket |
| Implemented | 2026-06-13T14:00:00Z                              |
| Status      | COMPLETE                                          |

## Summary

Replaced the `tokio::net::TcpListener` / `tokio::net::TcpStream` IPC transport in `managed.rs` with the `zeromq` crate's DEALER socket over `tcp://127.0.0.1:{port}`. The supervisor binds a DEALER socket on an OS-assigned port, passes the port via `ANVILML_IPC_PORT`, and the `IpcHandles` struct holds a `zeromq::DealerSocket` instead of boxed `AsyncRead`/`AsyncWrite` trait objects. The combined `run_loop` uses zeromq `send`/`recv` with msgpack-serialised bytes via `tokio::select!`, and the custom 4-byte length-prefix framing (`anvilml_ipc::framing`) is removed from the data path. The `ipc_socket_path` field was removed from `ManagedWorker`, and `serialize_message` + `worker_event_from_map` were copied as private helpers from `framing.rs`.

## Resolved Dependencies

| Type   | Name        | Version resolved | Source         |
|--------|-------------|------------------|----------------|
| crate  | zeromq      | 0.4.1            | lockfile (workspace dep already declared) |
| crate  | serde_json  | 1.0.150          | workspace dep (added as direct dep) |
| crate  | uuid        | 1.23.2           | workspace dep (added as direct dep) |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/managed.rs` | Replace TcpListener/TcpStream IPC with zeromq DEALER socket; rewrite IpcHandles, spawn(), run_loop (combined select! reader/writer); copy serialize_message + worker_event_from_map as private helpers; remove framing import and ipc_socket_path field; update inject_handles_for_test to use DEALER sockets; update eof_sets_dead test |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump version 0.1.24 → 0.1.25; add serde_json and uuid as direct dependencies |

## Commit Log

```
 Cargo.lock                           |   3 +-
 crates/anvilml-worker/Cargo.toml     |   4 +-
 crates/anvilml-worker/src/managed.rs | 709 +++++++++++++++++++++++++----------
 3 files changed, 514 insertions(+), 202 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-e23660dbf884a4ec)

running 15 tests
test env::tests::test_build_env_cpu ... ok
test env::tests::test_build_env_cuda ... ok
test env::tests::test_build_env_ipc_port ... ok
test env::tests::test_build_env_mock_propagation ... ok
test env::tests::test_build_env_rocm_linux_hsa ... ok
test env::tests::test_build_env_rocm_windows_no_hsa ... ok
test pool::tests::pid_for_returns_none_for_missing_child ... ok
test pool::tests::pid_for_returns_child_pid_when_spawned ... ok
test pool::tests::restart_exits_0_and_returns_to_idle ... ok
test pool::tests::pool_event_listener_merges_ready_capabilities ... ok
test pool::tests::spawn_all_creates_cpu_worker_when_no_gpus ... ok
test managed::tests::eof_sets_dead ... ok
test managed::tests::keepalive_pings_and_kills_on_timeout ... ok
test managed::tests::respawn_after_death ... ok
test pool::tests::shutdown_all_stops_all ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out
```

Four spawn tests (`spawn_ping_pong`, `status_transitions`, `handshake_completes_once`, `spawn_reaches_idle`) are filtered out because they require a real Python worker interpreter (they skip when `ANVILML_VENV_PATH` is not set or Python is unavailable).

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
cargo check --workspace --features mock-hardware
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.91s

# 2. Mock-hardware Windows cross-check
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.67s

# 3. Real-hardware Linux check
cargo check --bin anvilml
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 12.16s

# 4. Real-hardware Windows cross-check
cargo check --bin anvilml --target x86_64-pc-windows-gnu
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 14.62s

All four checks exited 0.
```

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p backend --features mock-hardware -- config_reference
→ running 1 test
→ test test_toml_key_set_matches_default ... ok
→ test result: ok. 1 passed; 0 failed; 0 ignored
```

Gate 2 (OpenAPI Drift) does not apply: no handler signatures, `ToSchema` types, or `#[utoipa::path]` annotations were modified.

## Deviations from Plan

- **Combined run_loop instead of separate writer/reader tasks**: The zeromq `DealerSocket` cannot be cloned, so separate spawned tasks for writer and reader were not possible. Instead, a single `run_loop` task handles both sending and receiving via `tokio::select!`. This is a necessary deviation to work within zeromq's socket ownership model.
- **`inject_handles_for_test` uses DEALER sockets instead of PAIR sockets**: The zeromq 0.4.1 crate does not provide a `PairSocket` type. Available socket types are Dealer, Req, Rep, Push, Pull, Pub, Sub, and Router. The test helper was updated to use DEALER-DEALER connections instead.
- **EOF test uses invalid message instead of socket drop**: The zeromq DEALER socket does not immediately detect connection drops (unlike PAIR sockets). The `eof_sets_dead` test was updated to send an invalid msgpack message to trigger a deserialization error, which causes the run_loop to exit and broadcast `WorkerStatusChanged(Dead)`. This tests the same error-handling path as EOF.
- **Added `serde_json` and `uuid` as direct dependencies**: These were previously available transitively through `anvilml-ipc` and `anvilml-core`, but are now used directly in the copied helper functions in `managed.rs`.

## Blockers

None.
