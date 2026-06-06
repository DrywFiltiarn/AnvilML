# Implementation Report: P9-A2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P9-A2                                             |
| Phase       | 009 — Worker Spawn & Handshake                    |
| Description | worker: worker_main.py mock-mode message loop (Ping/Pong/Init/Shutdown) |
| Implemented | 2026-06-06T14:30:00Z                              |
| Status      | COMPLETE                                          |

## Summary

Implemented `worker/worker_main.py`, the Python worker entry point implementing a blocking
stdin/stdout message loop over the IPC framing protocol. Supports mock mode via
`ANVILML_WORKER_MOCK=1` which skips torch entirely and reports fixed stub GPU properties.
The worker handles `InitializeHardware` → `Ready{}`, `Ping` → `Pong{seq}`,
`MemoryQuery` → `MemoryReport{0,0}`, and `Shutdown` → `Dying{reason:shutdown}` + exit 0.
A background daemon thread emits `MemoryReport` every 10 seconds. Created corresponding
pytest tests in `worker/tests/test_worker_main.py` that spawn the worker as a subprocess
and verify all message/response pairs.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|-----------------|----------------|
| python | msgpack   | ≥1.0 (existing) | base.txt       |
| python | torch     | skipped in mock | conditional    |

No new dependencies added. The worker uses only `os`, `sys`, `argparse`, `threading`,
and `worker.ipc` (already in the repo). Torch is imported conditionally and skipped
entirely in mock mode.

## Files Changed

| Action | Path                          | Description |
|--------|-------------------------------|-------------|
| Create | `worker/worker_main.py`       | Worker entry point: argparse, sys.path fix, thread env setup, conditional torch import, hardware probe (mock/real), message loop (InitializeHardware/Ping-Pong/MemoryQuery-MemoryReport/Shutdown-Dying), background MemoryReport thread. |
| Create | `worker/tests/test_worker_main.py` | pytest: 5 integration tests spawning worker subprocess in mock mode — InitializeHardware→Ready, mock values, Ping→Pong, MemoryQuery→MemoryReport, Shutdown→Dying+exit0. |

## Commit Log

```
 worker/tests/test_worker_main.py | 187 ++++++++++++++++++++++++++++++++++
 worker/worker_main.py            | 212 +++++++++++++++++++++++++++++++++++++++
 2 files changed, 399 insertions(+)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.0.3, pluggy-1.6.0 -- /home/dryw/forge/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML
plugins: anyio-4.12.1
collecting ... collected 10 items

worker/tests/test_ipc.py::TestReadFrame::test_write_read_roundtrip PASSED [ 10%]
worker/tests/test_ipc.py::TestReadFrame::test_roundtrip_with_bytes PASSED [ 20%]
worker/tests/test_ipc.py::TestReadFrame::test_roundtrip_empty_dict PASSED [ 30%]
worker/tests/test_ipc.py::TestWindowsGuard::test_windows_binary_mode_guard_present SKIPPED [ 40%]
worker/tests/test_ipc.py::TestWindowsGuard::test_guard_code_exists_in_source PASSED [ 50%]
worker/tests/test_worker_main.py::TestWorkerMain::test_ready_on_init_hardware PASSED [ 60%]
worker/tests/test_worker_main.py::TestWorkerMain::test_mock_values PASSED [ 70%]
worker/tests/test_worker_main.py::TestWorkerMain::test_ping_pong PASSED [ 80%]
worker/tests/test_worker_main.py::TestWorkerMain::test_memory_query_report PASSED [ 90%]
worker/tests/test_worker_main.py::TestWorkerMain::test_shutdown_dying_exit PASSED [100%]

========================= 9 passed, 1 skipped in 0.27s =========================
```

Rust tests: all 198 tests passed (74 anvilml_core + 56 anvilml_hardware + 23 anvilml_ipc +
19 anvilml_registry + 8 anvilml_server + 8 anvilml main + 1 config_reference + 4 device_store
+ 2 rescan + 1 scanner + 7 seed_loader + 2 store_get + 3 store_list + 2 doc-tests).

## Format Gate

```
(cargo fmt --all -- --check exited with code 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.31s

# 2. Mock-hardware Windows cross-check
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.32s

# 3. Real-hardware Linux check
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s

# 4. Real-hardware Windows cross-check
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.27s
```

All four platform cross-checks exit 0.

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p backend --features mock-hardware
     Running tests/config_reference.rs
     running 1 test
     test test_toml_key_set_matches_default ... ok
     test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

# Real-hardware clippy
cargo clippy --bin anvilml -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.33s
```

## Deviations from Plan

- Added `sys.path.insert(0, _repo_root)` at module top of `worker_main.py` to ensure
  `worker.ipc` is importable regardless of how the script is invoked (direct path vs
  `-m`). This was not in the plan but is necessary for subprocess testing to work.
- Test approach uses direct script execution (`python worker/worker_main.py`) rather than
  `python -m worker.worker_main` because the latter fails when the subprocess cwd is the
  `worker/` directory (Python cannot locate the `worker` package). The test sends all IPC
  frames at once and reads stdout after closing stdin, avoiding blocking on `proc.stdout.read()`.

## Blockers

None. All tests pass, all gates clear, all cross-checks exit 0.
