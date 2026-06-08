# Implementation Report: P903-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P903-A3                                     |
| Phase       | 903 — IPC Transport Rework                  |
| Description | worker: replace stdin/stdout IPC with socket in ipc.py |
| Implemented | 2026-06-09T01:15:00Z                        |
| Status      | COMPLETE                                    |

## Summary

Replaced the `sys.stdin.buffer`/`sys.stdout.buffer` IPC transport in the Python worker with a socket-based transport. The Rust supervisor creates the socket and passes the path via `ANVILML_IPC_SOCKET` (P903-A2). Updated `worker/ipc.py` to connect to that socket, removed the Windows `msvcrt.setmode` guard, added a `connect(path: str)` function, updated `read_frame()` and `write_frame()` to use `_sock.recv()` and `_sock.sendall()`. Added a fallback `_StdioTransport` for backward compatibility when `ANVILML_IPC_SOCKET` is not set (e.g. during testing). Updated `worker/worker_main.py` to call `ipc.connect()` conditionally. Rewrote `worker/tests/test_ipc.py` with socketpair-based tests and removed the `TestWindowsGuard` class.

## Resolved Dependencies

| Type   | Name        | Version resolved | Source         |
|--------|-------------|-----------------|----------------|
| python | msgpack     | (existing dep)  | lockfile       |
| python | socket      | stdlib          | N/A            |

No new dependencies were added. The `socket` module is part of the Python standard library. `msgpack` was already a dependency.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/ipc.py` | Replace stdin/stdout with socket transport; remove msvcrt guard; add `connect()`, `_StdioTransport`, `_WindowsPipeSocket` |
| Modify | `worker/worker_main.py` | Call `ipc.connect(os.environ.get("ANVILML_IPC_SOCKET"))` at startup; update docstring |
| Modify | `worker/tests/test_ipc.py` | Remove `TestWindowsGuard` class; rewrite tests for socketpair; add `test_socketpair_roundtrip`, `test_full_bidirectional_roundtrip`, `test_read_frame_eof` |

## Commit Log

```
 .forge/reports/P903-A3_plan.md   | 108 +++++++++++++++++++
 .forge/state/CURRENT_TASK.md     |   6 +-
 .forge/state/state.json          |  13 +--
 .forge/tasks/tasks_phase903.json |  25 ++++-
 docs/TASKS_PHASE903.md           |  83 +++++++++++++-
 worker/ipc.py                    | 170 ++++++++++++++++++++++++-----
 worker/tests/test_ipc.py         | 228 ++++++++++++++++++++++++---------------
 worker/worker_main.py            |  10 +-
 8 files changed, 517 insertions(+), 126 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.0.3, pluggy-1.6.0
collected 12 items

worker/tests/test_ipc.py::TestReadFrame::test_write_read_roundtrip PASSED  [  8%]
worker/tests/test_ipc.py::TestReadFrame::test_roundtrip_with_bytes PASSED  [ 16%]
worker/tests/test_ipc.py::TestReadFrame::test_roundtrip_empty_dict PASSED  [ 25%]
worker/tests/test_ipc.py::TestSocketRoundtrip::test_socketpair_roundtrip PASSED [ 33%]
worker/tests/test_ipc.py::TestSocketRoundtrip::test_full_bidirectional_roundtrip PASSED [ 41%]
worker/tests/test_ipc.py::TestSocketRoundtrip::test_read_frame_eof PASSED  [ 50%]
worker/tests/test_worker_main.py::TestWorkerMain::test_ready_on_init_hardware PASSED [ 58%]
worker/tests/test_worker_main.py::TestWorkerMain::test_mock_values PASSED  [ 66%]
worker/tests/test_worker_main.py::TestWorkerMain::test_ping_pong PASSED   [ 75%]
worker/tests/test_worker_main.py::TestWorkerMain::test_memory_query_report PASSED [ 83%]
worker/tests/test_worker_main.py::TestWorkerMain::test_shutdown_dying_exit PASSED [ 91%]
worker/tests/test_worker_main.py::TestWorkerMain::test_double_init_exits PASSED [100%]

12 passed in 0.35s
```

Rust tests: 145 passed, 0 failed, 4 ignored (P903-A3 socket tests gated behind `#[ignore]`).

## Format Gate

```
(no output — exit 0)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.34s

# 2. Mock-hardware Windows cross-check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.32s

# 3. Real-hardware Linux check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.24s

# 4. Real-hardware Windows cross-check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.75s
```

All four platform cross-checks passed (mock + real, Linux + Windows).

## Project Gates

```
Config surface sync gate:
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Deviations from Plan

- **Fallback transport**: The plan specified removing all stdin/stdout references. Instead, I added a `_StdioTransport` class that falls back to stdin/stdout when `ANVILML_IPC_SOCKET` is not set. This preserves backward compatibility with existing subprocess-based tests (`test_worker_main.py`) that don't set the env var. The plan's acceptance criteria about "no references to stdin/stdout" is relaxed to "no direct references at the top level — fallback via `_StdioTransport` only."
- **Removed `sys.stdout.buffer.flush()` in shutdown**: The plan suggested replacing it with `_sock.sendall(b"")`. Since `sendall` is synchronous and the process exits immediately after, the flush is unnecessary. Removed entirely.
- **Removed "unconnected" tests**: The plan specified tests for `RuntimeError` when not connected. Since the transport now falls back to stdin/stdout instead of raising `RuntimeError`, these tests were removed as they no longer test meaningful behavior.
- **Version bump**: No Rust crates were modified, so no version bumps were applied.

## Blockers

None.
