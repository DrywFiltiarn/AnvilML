# Implementation Report: P9-D3

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P9-D3                           |
| Phase         | 009 — Real Worker Startup       |
| Description   | worker_main.py: mock-mode startup sequence |
| Implemented   | 2026-07-05T18:15:00Z            |
| Status        | COMPLETE                        |

## Summary

This was a verification-only task confirming that the mock-mode startup sequence in `worker/worker_main.py` is fully implemented per `ANVILML_DESIGN.md §14.3`. The implementation was already completed in prior tasks (P9-C2 for `_mock_probe_capabilities()`, P9-D1/P9-D2 for the unified `__main__` dispatch). All verification checks passed: the startup sequence follows the correct step order, `_mock_probe_capabilities()` returns exactly 6 bool keys with `fp4=False`, the `__main__` block dispatches via a single `== "1"` check, test counts exceed requirements (14 total, 10 real_mode, 4 unmarked), the torch-import test uses subprocess isolation with `timeout=10`, no mock-only gate exists, and dual-mode parity markers do not apply to these startup-sequence helper functions.

## Resolved Dependencies

None. This task verified existing code; no new dependencies were introduced or modified.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Verify (no changes) | `worker/worker_main.py` | Mock-mode startup sequence already implemented; all checks passed |
| Verify (no changes) | `worker/tests/test_worker_main.py` | Test suite already present; all counts verified |

No files were created or modified. This task is a verification/confirmation task — the mock-mode startup sequence was already implemented in prior tasks.

## Commit Log

```
 .forge/state/CURRENT_TASK.md |  6 +++---
 .forge/state/state.json      | 13 +++++++------
 2 files changed, 10 insertions(+), 9 deletions(-)
```

Only `.forge/` state files were changed (orchestrator bookkeeping). No source files were modified.

## Test Results

### Rust tests (full workspace, --features mock-hardware)
```
     Running unittests src/lib.rs (target/debug/deps/anvilml-7708e8b7193f85b4)
running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/cli_help_test.rs
running 1 test
test tests::cli_help_shows_all_flags ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_reference.rs
running 1 test
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/db_startup_tests.rs
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/hw_probe_help_test.rs
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/logging_tests.rs
running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/shutdown_tests.rs
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/store_tests.rs (anvilml_artifacts)
running 9 tests
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_load_tests.rs (anvilml_core)
running 13 tests
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/config_tests.rs (anvilml_core)
running 13 tests
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/error_tests.rs (anvilml_core)
running 16 tests
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/events_tests.rs (anvilml_core)
running 10 tests
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/hardware_tests.rs (anvilml_core)
running 9 tests
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/job_tests.rs (anvilml_core)
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/model_tests.rs (anvilml_core)
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/node_registry_tests.rs (anvilml_core)
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/node_tests.rs (anvilml_core)
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/worker_tests.rs (anvilml_core)
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/cpu_tests.rs (anvilml_hardware)
running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/detect_tests.rs (anvilml_hardware)
running 14 tests
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/mock_tests.rs (anvilml_hardware)
running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/sysfs_tests.rs (anvilml_hardware)
running 7 tests
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/vulkan_tests.rs (anvilml_hardware)
running 8 tests
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/error_tests.rs (anvilml_ipc)
running 7 tests
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/roundtrip_tests.rs (anvilml_ipc)
running 26 tests
test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/stress_test.rs (anvilml_ipc)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/db_tests.rs (anvilml_registry)
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/device_store_tests.rs (anvilml_registry)
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/scanner_tests.rs (anvilml_registry)
running 20 tests
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/seed_loader_tests.rs (anvilml_registry)
running 8 tests
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/store_tests.rs (anvilml_registry)
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/health_tests.rs (anvilml_server)
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/bridge_tests.rs (anvilml_worker)
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/demux_tests.rs (anvilml_worker)
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/env_tests.rs (anvilml_worker)
running 7 tests
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/keepalive_tests.rs (anvilml_worker)
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/managed_tests.rs (anvilml_worker)
running 39 tests
test result: ok. 39 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/pool_tests.rs (anvilml_worker)
running 5 tests
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/respawn_tests.rs (anvilml_worker)
running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

     Running tests/spawn_tests.rs (anvilml_worker)
running 6 tests
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_registry
running 2 tests
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

   Doc-tests anvilml_worker
running 1 test
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

all doctests ran in 1.15s; merged doctests compilation took 1.10s

=== Rust total: ALL PASSED ===
```

### Python mock-mode tests (not real_mode)
```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0
collected 29 items / 19 deselected / 10 selected

worker/tests/test_ipc.py::TestConnectIdentity::test_connect_sets_identity PASSED
worker/tests/test_ipc.py::TestPreConnectErrors::test_send_event_before_connect_raises PASSED
worker/tests/test_ipc.py::TestPreConnectErrors::test_recv_message_before_connect_raises PASSED
worker/tests/test_ipc.py::TestRoundtrip::test_roundtrip_send_recv PASSED
worker/tests/test_ipc.py::TestNoTorchImport::test_module_no_torch_import PASSED
worker/tests/test_ipc.py::TestContextReuse::test_connect_twice_reuses_context PASSED
worker/tests/test_worker_main.py::TestMockProbeCapabilities::test_returns_six_required_keys PASSED
worker/tests/test_worker_main.py::TestMockProbeCapabilities::test_all_values_are_bool PASSED
worker/tests/test_worker_main.py::TestMockProbeCapabilities::test_fp4_is_false PASSED
worker/tests/test_worker_main.py::TestNoTorchImport::test_no_torch_import_on_module_load PASSED

====================== 10 passed, 19 deselected in 5.38s =======================

=== Python mock-mode total: 10 passed, 0 failed ===
```

### Python real-mode tests (real_mode)
```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0
collected 29 items / 10 deselected / 19 selected

worker/tests/test_capability.py::TestProbeDtypes::test_fp32_cpu_returns_true PASSED
worker/tests/test_capability.py::TestProbeDtypes::test_fp16_cpu_returns_true PASSED
worker/tests/test_capability.py::TestProbeDtypes::test_bf16_cpu_returns_true PASSED
worker/tests/test_capability.py::TestProbeDtypes::test_fp8_cpu_returns_false PASSED
worker/tests/test_capability.py::TestProbeDtypes::test_fp4_cpu_returns_false PASSED
worker/tests/test_capability.py::TestProbeFlashAttention::test_flash_attention_cpu_returns_true PASSED
worker/tests/test_capability.py::TestProbeStructure::test_returns_dict_with_exactly_six_bool_keys PASSED
worker/tests/test_capability.py::TestProbeStructure::test_never_raises_for_cpu PASSED
worker/tests/test_capability.py::TestProbeStructure::test_device_selection_cpu PASSED
worker/tests/test_worker_main.py::TestRealStartupSequence::test_real_startup_calls_ipc_connect PASSED
worker/tests/test_worker_main.py::TestRealStartupSequence::test_real_startup_cpu_skips_cuda_set_device PASSED
worker/tests/test_worker_main.py::TestRealStartupSequence::test_real_startup_calls_probe_capabilities PASSED
worker/tests/test_worker_main.py::TestNoMockGate::test_no_mock_gate_exit_path PASSED
worker/tests/test_worker_main.py::TestNoMockGate::test_real_startup_sends_ready_event PASSED
worker/tests/test_worker_main.py::TestNoMockGate::test_import_nodes_returns_empty_list PASSED
worker/tests/test_worker_main.py::TestNoMockGate::test_dispatch_loop_exists_and_is_callable PASSED
worker/tests/test_worker_main.py::TestNoMockGate::test_real_startup_no_nonzero_exit_for_cpu PASSED
worker/tests/test_worker_main.py::TestNoMockGate::test_mock_startup_sends_ready_event PASSED
worker/tests/test_worker_main.py::TestNoMockGate::test_no_mock_gate_in_main_block PASSED

====================== 19 passed, 10 deselected in 2.07s =======================

=== Python real-mode total: 19 passed, 0 failed ===
```

## Format Gate

```
cargo fmt --all -- --check
(exited 0 — no output, all files already formatted)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux (primary platform — already verified in Rust tests above)
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 25.27s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 31.95s

# 3. Real-hardware Linux
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 28.47s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 26.13s

All four platform cross-checks exited 0.
```

## Project Gates

### Gate 1 — Config Surface Sync
```
cargo test -p anvilml --features mock-hardware -- config_reference
running 1 test
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Gate — Clippy (real-hardware)
```
cargo clippy --bin anvilml -- -D warnings
Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.77s
```

## Public API Delta

No source files were modified in this task. No new `pub` items were introduced.

```
(grep returned nothing — no modifications to public API)
```

## Deviations from Plan

None. The implementation matches the plan exactly. All verification checks passed:

1. **`_mock_startup_sequence()` structure**: Confirmed correct step order (lines 171–205): read env vars → `ipc.connect()` → `_mock_probe_capabilities()` → `_import_nodes()` → `ipc.send_event(Ready{capabilities_source="mock", node_types:[]})` → `_dispatch_loop()`.

2. **`_mock_probe_capabilities()` correctness**: Confirmed returns dict with exactly 6 keys (`fp32`, `fp16`, `bf16`, `fp8`, `fp4`, `flash_attention`), all bool values, `fp4=False`, others `True` (lines 208–249).

3. **`__main__` dispatch logic**: Confirmed single check at line 260: `os.environ.get("ANVILML_WORKER_MOCK") == "1"`, dispatching to `_mock_startup_sequence()` (line 262) or `_real_startup_sequence()` (line 265).

4. **Test counts**: 14 total `def test_` methods in `test_worker_main.py` (≥11 requirement met). 10 `@pytest.mark.real_mode` tests (≥8). 4 unmarked tests (≥4).

5. **Torch-isolation test**: Confirmed `test_no_torch_import_on_module_load` uses `subprocess.run()` with `timeout=10` (lines 93–103), not `sys.modules.pop`.

6. **No mock-only gate**: Confirmed no line in `worker_main.py` matches `if ANVILML_WORKER_MOCK != "1": exit(1)`. The `test_no_mock_gate_in_main_block` test performs this mechanical check.

7. **Dual-mode parity markers**: Confirmed not applicable — `worker_main.py` contains only startup-sequence helpers (`_mock_startup_sequence`, `_real_startup_sequence`, `_mock_probe_capabilities`), none of which are node `execute()` or arch module `load()`/`sample()`/`decode()` functions.

8. **Phase-closing audit**: Confirmed not applicable — P9-D3 is not the last task in `tasks_phase009.json`; P9-E1 and P9-F1 follow it.

## Blockers

None.
