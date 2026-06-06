# Implementation Report: P9-B2

| Field         | Value                                           |
|---------------|-------------------------------------------------|
| Task ID       | P9-B2                                           |
| Phase         | 009 — CI: Python Worker                         |
| Description   | ci: add python-worker job (Linux + Windows pytest, ANVILML_WORKER_MOCK=1) |
| Implemented   | 2026-06-06T16:00:00Z                            |
| Status        | COMPLETE                                        |

## Summary

Added a new `python-worker` CI job to `.github/workflows/ci.yml` that runs on both
`ubuntu-latest` and `windows-latest` using a matrix strategy. The job checks out the
repository, installs Python 3.12 via `actions/setup-python@v5`, installs the three test
dependencies (`msgpack`, `pillow`, `pytest`) directly (no venv), and runs the Python worker
test suite under `ANVILML_WORKER_MOCK=1`. All existing Rust CI jobs (`rust-linux`,
`rust-windows`) are preserved unchanged. No Rust source files were modified, so no version
bumps were required.

## Resolved Dependencies

No new dependencies added. The three Python packages (`msgpack`, `pillow`, `pytest`) are
already established in `worker/requirements/base.txt`. No MCP tool call required per plan.

## Files Changed

| Action | Path                        | Description                                              |
|--------|-----------------------------|----------------------------------------------------------|
| Modify | `.github/workflows/ci.yml`  | Append `python-worker` job (19 lines, matrix strategy)   |

No Rust source files modified — no version bumps applied.

## Commit Log

```
.forge/state/CURRENT_TASK.md |  6 +++---
 .forge/state/state.json      | 13 +++++++------
 .github/workflows/ci.yml     | 19 +++++++++++++++++++
 3 files changed, 29 insertions(+), 9 deletions(-)
```

## Test Results

### Rust tests (cargo test --workspace --features mock-hardware)

```
test result: ok. 74 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  [anvilml-core]
test result: ok. 56 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  [anvilml-hardware]
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  [anvilml-ipc]
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   [ipc_probe]
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   [anvilml-openapi]
test result: ok. 19 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  [anvilml-registry]
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   [anvilml_registry_db]
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   [device_store]
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   [rescan]
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   [scanner]
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   [seed_loader]
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   [store_get]
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   [store_list]
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   [anvilml-scheduler]
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   [anvilml-server]
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   [api_models]
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   [api_ws_events]
test result: ok. 8 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out   [anvilml-worker]
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   [anvilml binary]
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   [config_reference]
  Doc-tests anvilml_core: 0 passed; 0 failed
   Doc-tests anvilml_hardware: 2 passed; 0 failed
   Doc-tests anvilml_ipc: 0 passed; 0 failed
   Doc-tests anvilml_registry: 0 passed; 0 failed
   Doc-tests anvilml_scheduler: 0 passed; 0 failed
   Doc-tests anvilml_server: 0 passed; 0 failed
   Doc-tests anvilml_worker: 0 passed; 0 failed

Total: 218 passed, 0 failed, 2 ignored
```

### Python worker tests (ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v)

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.0.3, pluggy-1.6.0
rootdir: /home/dryw/AnvilML
plugins: anyio-4.12.1
collected 10 items

worker/tests/test_ipc.py::TestReadFrame::test_write_read_roundtrip PASSED
worker/tests/test_ipc.py::TestReadFrame::test_roundtrip_with_bytes PASSED
worker/tests/test_ipc.py::TestReadFrame::test_roundtrip_empty_dict PASSED
worker/tests/test_ipc.py::TestWindowsGuard::test_windows_binary_mode_guard_present SKIPPED
worker/tests/test_ipc.py::TestWindowsGuard::test_guard_code_exists_in_source PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_ready_on_init_hardware PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_mock_values PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_ping_pong PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_memory_query_report PASSED
worker/tests/test_worker_main.py::TestWorkerMain::test_shutdown_dying_exit PASSED

========================= 9 passed, 1 skipped in 0.28s =========================
```

## Format Gate

```
(cargo fmt --all -- --check — exit 0, no drift)
```

## Platform Cross-Check

All four checks passed:

1. `cargo check --workspace --features mock-hardware` → Finished (exit 0)
2. `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` → Finished (exit 0)
3. `cargo check --bin anvilml` → Finished (exit 0)
4. `cargo check --bin anvilml --target x86_64-pc-windows-gnu` → Finished (exit 0)

## Project Gates

Gate 1 — Config Surface Sync: `cargo test -p backend --features mock-hardware -- config_reference`
→ exit 0, no failures. (Note: this task modifies only CI workflow files, not config structs,
so Gate 1 is not applicable to the scope of changes.)

## Deviations from Plan

None. Implementation matches the approved plan exactly.

## Blockers

None.
