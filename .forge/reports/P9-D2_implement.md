# Implementation Report: P9-D2

| Field         | Value                                           |
|---------------|-------------------------------------------------|
| Task ID       | P9-D2                                           |
| Phase         | 009 — Real Worker Startup                       |
| Description   | worker_main.py: real-mode node-import stub + Ready event + loop |
| Implemented   | 2026-07-05T17:30:00Z                            |
| Status        | COMPLETE                                        |

## Summary

Completed the real-mode startup sequence in `worker/worker_main.py` by adding three module-level functions (`_import_nodes()`, `_dispatch_loop()`, `_mock_startup_sequence()`), extending `_real_startup_sequence()` to send a `Ready` event with `capabilities_source="pytorch"` and enter the dispatch loop, and updating the `__main__` block to dispatch between mock and real mode based on `ANVILML_WORKER_MOCK`. Added 6 new real_mode-marked tests and updated 3 existing tests to mock the new `send_event` call. Total real_mode test count is now 10 (4 existing + 6 new), satisfying the >=7 requirement.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source         |
|--------|---------|------------------|----------------|
| (none) |         |                  |                |

This task introduces no new dependencies. It uses only existing dependencies (`zmq`, `msgpack` via `worker.ipc`) and standard library modules (`logging`, `os`, `sys`).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/worker_main.py` | Added `_import_nodes()`, `_dispatch_loop()`, `_mock_startup_sequence()`; extended `_real_startup_sequence()` to send Ready event and enter dispatch loop; updated `__main__` block with mock/real dispatcher |
| Modify | `worker/tests/test_worker_main.py` | Added 6 new real_mode-marked tests; updated 3 existing tests to mock `ipc.send_event` and `ipc.recv_message` |
| Modify | `docs/TESTS.md` | Added 6 new test entries for P9-D2 tests |

## Commit Log

```
 .forge/reports/P9-D2_plan.md     | 293 ++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md     |   6 +-
 .forge/state/state.json          |  13 +-
 docs/TESTS.md                    |  72 +++++++
 worker/tests/test_worker_main.py | 434 +++++++++++++++++++++++++++++++++++----
 worker/worker_main.py            | 166 +++++++++++++--
 6 files changed, 920 insertions(+), 64 deletions(-)
```

## Test Results

### Mock-mode tests (10 passed)

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 29 items / 19 deselected / 10 selected

worker/tests/test_ipc.py::TestConnectIdentity::test_connect_sets_identity PASSED [ 10%]
worker/tests/test_ipc.py::TestPreConnectErrors::test_send_event_before_connect_raises PASSED [ 20%]
worker/tests/test_ipc.py::TestPreConnectErrors::test_recv_message_before_connect_raises PASSED [ 30%]
worker/tests/test_ipc.py::TestRoundtrip::test_roundtrip_send_recv PASSED [ 40%]
worker/tests/test_ipc.py::TestNoTorchImport::test_module_no_torch_import PASSED [ 50%]
worker/tests/test_ipc.py::TestContextReuse::test_connect_twice_reuses_context PASSED [ 60%]
worker/tests/test_worker_main.py::TestMockProbeCapabilities::test_returns_six_required_keys PASSED [ 70%]
worker/tests/test_worker_main.py::TestMockProbeCapabilities::test_all_values_are_bool PASSED [ 80%]
worker/tests/test_worker_main.py::TestMockProbeCapabilities::test_fp4_is_false PASSED [ 90%]
worker/tests/test_worker_main.py::TestNoTorchImport::test_no_torch_import_on_module_load PASSED [100%]

====================== 10 passed, 19 deselected in 5.50s =======================
```

### Real-mode tests (19 passed)

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 29 items / 10 deselected / 19 selected

worker/tests/test_capability.py::TestProbeDtypes::test_fp32_cpu_returns_true PASSED [  5%]
worker/tests/test_capability.py::TestProbeDtypes::test_fp16_cpu_returns_true PASSED [ 10%]
worker/tests/test_capability.py::TestProbeDtypes::test_bf16_cpu_returns_true PASSED [ 15%]
worker/tests/test_capability.py::TestProbeDtypes::test_fp8_cpu_returns_false PASSED [ 21%]
worker/tests/test_capability.py::TestProbeDtypes::test_fp4_cpu_returns_false PASSED [ 26%]
worker/tests/test_capability.py::TestProbeFlashAttention::test_flash_attention_cpu_returns_true PASSED [ 31%]
worker/tests/test_capability.py::TestProbeStructure::test_returns_dict_with_exactly_six_bool_keys PASSED [ 36%]
worker/tests/test_capability.py::TestProbeStructure::test_never_raises_for_cpu PASSED [ 42%]
worker/tests/test_capability.py::TestProbeStructure::test_device_selection_cpu PASSED [ 47%]
worker/tests/test_worker_main.py::TestRealStartupSequence::test_real_startup_calls_ipc_connect PASSED [ 52%]
worker/tests/test_worker_main.py::TestRealStartupSequence::test_real_startup_cpu_skips_cuda_set_device PASSED [ 57%]
worker/tests/test_worker_main.py::TestRealStartupSequence::test_real_startup_calls_probe_capabilities PASSED [ 63%]
worker/tests/test_worker_main.py::TestNoMockGate::test_no_mock_gate_exit_path PASSED [ 68%]
worker/tests/test_worker_main.py::TestNoMockGate::test_real_startup_sends_ready_event PASSED [ 73%]
worker/tests/test_worker_main.py::TestNoMockGate::test_import_nodes_returns_empty_list PASSED [ 78%]
worker/tests/test_worker_main.py::TestNoMockGate::test_dispatch_loop_exists_and_is_callable PASSED [ 84%]
worker/tests/test_worker_main.py::TestNoMockGate::test_real_startup_no_nonzero_exit_for_cpu PASSED [ 89%]
worker/tests/test_worker_main.py::TestNoMockGate::test_mock_startup_sends_ready_event PASSED [ 94%]
worker/tests/test_worker_main.py::TestNoMockGate::test_no_mock_gate_in_main_block PASSED [100%]

====================== 19 passed, 10 deselected in 2.04s =======================
```

### Rust tests (244 passed)

All 244 Rust tests passed across the full workspace with `--features mock-hardware`.

## Format Gate

```
(no output — cargo fmt --all -- --check exited 0)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.33s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 30.95s

# 3. Real-hardware Linux
cargo check --bin anvilml
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 31.45s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 27.72s
```

All four platform cross-checks passed.

## Project Gates

Gate 1 (config_reference), Gate 2 (openapi-drift), Gate 3 (node_parity), and Gate 4 (mock/real parity markers) are not triggered by this task — it does not modify `ServerConfig`, handler functions, node types, or node `execute()`/arch module `load()`/`sample()`/`decode()` functions.

## Public API Delta

```
+def _import_nodes() -> list:
+def _dispatch_loop() -> None:
+def _real_startup_sequence() -> None:
+def _mock_startup_sequence() -> None:
```

All new items are private module-level functions (prefixed with `_`). No new public API items. The `_real_startup_sequence()` return type changed from `dict` to `None`.

## Deviations from Plan

1. **`_dispatch_loop()` now imports `ipc` internally** — The plan showed `_dispatch_loop()` calling `ipc.recv_message()` without importing `ipc`. Since `ipc` is only imported inside `_real_startup_sequence()` and `_mock_startup_sequence()`, `_dispatch_loop()` needs its own `import worker.ipc as ipc` at the top of the function body. This is the established pattern in the module.

2. **Existing tests updated to mock `ipc.send_event`** — The 3 existing real_mode tests (`test_real_startup_calls_ipc_connect`, `test_real_startup_cpu_skips_cuda_set_device`, `test_real_startup_calls_probe_capabilities`) were updated to mock `ipc.send_event` and `ipc.recv_message`. This was necessary because `_real_startup_sequence()` now calls `ipc.send_event()`, and the tests mock `ipc.connect` but the real `send_event` checks `_sock is None` and raises.

3. **Tests placed in `TestNoMockGate` class** — The 6 new tests were placed in the existing `TestNoMockGate` class (after the existing `test_no_mock_gate_exit_path` test) rather than a separate class. This is because `test_no_mock_gate_exit_path` already tests the `__main__` block structure, and the new `test_no_mock_gate_in_main_block` is a natural sibling. The other new tests (ready event, node import, dispatch loop, full startup) are logically related to the startup sequence and fit well in the same class.

## Blockers

None.
