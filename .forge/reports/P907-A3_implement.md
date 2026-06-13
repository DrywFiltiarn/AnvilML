# Implementation Report: P907-A3

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P907-A3                                           |
| Phase       | 907 — ZeroMQ IPC Transport                        |
| Description | anvilml-worker: env.rs replace ANVILML_IPC_SOCKET with ANVILML_IPC_PORT |
| Implemented | 2026-06-13T12:20:00Z                              |
| Status      | COMPLETE                                          |

## Summary

Replaced the `ANVILML_IPC_SOCKET` environment variable with `ANVILML_IPC_PORT` in
`anvilml-worker::env::build_worker_env`. The function signature now takes `ipc_port: u16`
instead of `ipc_socket_path: &str`, and the environment variable key changed from
`ANVILML_IPC_SOCKET` to `ANVILML_IPC_PORT`. The `managed.rs` call site was updated to
extract the port from `local_addr()` as a `u16` and pass it directly, while preserving
the `ipc_socket_path` field for logging. All 6 tests were updated and renamed
(`test_build_env_ipc_socket_path` → `test_build_env_ipc_port`). Crate version bumped
to `0.1.24`.

## Resolved Dependencies

| Type | Name | Version resolved | Source |
|------|------|------------------|--------|
| (none) | — | — | — |

No new dependencies added or modified.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/managed.rs` | Call site: extract `u16` port from `local_addr()`, pass to `build_worker_env`; keep `ipc_addr` string for logging |
| Modify | `crates/anvilml-worker/src/env.rs` | Function signature: `ipc_port: u16` (was `ipc_socket_path: &str`); env var key: `ANVILML_IPC_PORT`; update doc comment; update all 6 tests |
| Modify | `crates/anvilml-worker/Cargo.toml` | Bump patch version `0.1.23 → 0.1.24` |

## Commit Log

```
 crates/anvilml-worker/Cargo.toml     |  2 +-
 crates/anvilml-worker/src/env.rs     | 37 ++++++++++++++++++------------------
 crates/anvilml-worker/src/managed.rs |  5 +++--
 3 files changed, 23 insertions(+), 21 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_worker-dcb6eae36afe3bff)

running 6 tests
test env::tests::test_build_env_cuda ... ok
test env::tests::test_build_env_cpu ... ok
test env::tests::test_build_env_ipc_port ... ok
test env::tests::test_build_env_mock_propagation ... ok
test env::tests::test_build_env_rocm_windows_no_hsa ... ok
test env::tests::test_build_env_rocm_linux_hsa ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out; finished in 0.00s

   Doc-tests anvilml_worker

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## Format Gate

```
(No output — exit 0, no formatting drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.89s

# 2. Mock-hardware Windows cross-check
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.01s

# 3. Real-hardware Linux check
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 12.71s

# 4. Real-hardware Windows cross-check
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 15.30s
```

All four platform checks exited 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
     Running tests/config_reference.rs (target/debug/deps/config_reference-ee6f46dc8cac6caf)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s
```

Gate 1 passes (0 failures). No config surface changes in this task.

### Gate 2 — OpenAPI Drift
Not required — no files under `crates/anvilml-server/src/handlers/` modified, no
`ToSchema` types in `crates/anvilml-core/` changed, no `#[utoipa::path]` annotations
added or changed.

## Deviations from Plan

- After initial edit, clippy found that `ipc_addr` was still referenced in 3 logging
  lines (lines 215, 256, 273 of managed.rs). Added back a `let ipc_addr = format!("127.0.0.1:{}", ipc_port)`
  line to preserve the string for logging while passing the clean `u16` port to
  `build_worker_env`. This is consistent with the plan's intent to "preserve the
  `ipc_socket_path` field for logging."

## Blockers

None.
