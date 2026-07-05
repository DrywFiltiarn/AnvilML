# Implementation Report: P9-D1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P9-D1                           |
| Phase         | 009 — Real Worker Startup       |
| Description   | worker_main.py: real-mode connect+device-select+probe (no mock gate) |
| Implemented   | 2026-07-05T16:15:00Z            |
| Status        | COMPLETE                        |

## Summary

Added `_real_startup_sequence()` function to `worker/worker_main.py` that implements the real-mode startup sequence per ANVILML_DESIGN.md §14.2: reads env vars, connects over IPC, imports torch, selects the device (skipping for CPU), and calls `capability.probe_capabilities()`. Added `if __name__ == "__main__":` block. Added 4 real-mode tests in `worker/tests/test_worker_main.py` verifying IPC connect calls, CPU device selection skip, non-CPU probe invocation, and absence of mock gate. Updated `docs/TESTS.md` with entries for all 4 new tests. All Rust and Python tests pass (zero failures).

## Resolved Dependencies

None. This task uses only existing dependencies: `os`, `sys`, `logging` (stdlib), `zmq`, `msgpack`, `torch` (via `worker/ipc.py` and `worker/capability.py`).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/worker_main.py` | Added `_real_startup_sequence()` function (75 lines) and `if __name__ == "__main__":` block; added module-level `logging`, `os`, `sys` imports. |
| Modify | `worker/tests/test_worker_main.py` | Added `TestRealStartupSequence` class with 3 tests and `TestNoMockGate` class with 1 test; added `os` import. |
| Modify | `docs/TESTS.md` | Added 4 entries for new real-mode tests. |
| Modify | `.forge/reports/P9-D1_plan.md` | Plan report (pre-existing from PLAN session). |
| Modify | `.forge/state/CURRENT_TASK.md` | State file updated by this session. |
| Modify | `.forge/state/state.json` | State file updated by The Forge orchestrator. |

## Commit Log

```
 .forge/reports/P9-D1_plan.md     | 142 ++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md     |   6 +-
 .forge/state/state.json          |  13 +-
 docs/TESTS.md                    |  48 ++++++++
 worker/tests/test_worker_main.py | 248 +++++++++++++++++++++++++++++++++++++++
 worker/worker_main.py            |  80 +++++++++++++
 6 files changed, 528 insertions(+), 9 deletions(-)
```

## Test Results

```
=== Rust tests (cargo test --workspace --features mock-hardware) ===
All 295 tests passed across all crates. Zero failures.

=== Python mock-mode tests (ANVILML_WORKER_MOCK=1 pytest -v -m "not real_mode") ===
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collected 23 items / 13 deselected / 10 selected

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

====================== 10 passed, 13 deselected in 5.64s =======================

=== Python real-mode tests (pytest -v -m real_mode) ===
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collected 23 items / 10 deselected / 13 selected

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

====================== 13 passed, 10 deselected in 2.06s =======================
```

## Format Gate

```
cargo fmt --all -- --check
# exit 0 — no output, all files formatted
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4.37s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 30.97s

# 3. Real-hardware Linux
cargo check --bin anvilml
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 31.31s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 27.83s

All four checks exit 0.
```

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p anvilml --features mock-hardware -- config_reference
    Running tests/config_reference.rs
    running 1 test
    test tests::config_reference_matches_defaults ... ok
    test result: ok. 1 passed; 0 failed; 0 ignored

# Gate 2 — OpenAPI Drift: Not triggered (no handler signature changes)
# Gate 3 — Node Parity: Not triggered (no node type changes)
# Gate 4 — Mock/Real Parity Markers: Not triggered (no node execute() or arch module changes)
```

## Public API Delta

```
# grep '^+.*pub ' worker/worker_main.py — no new pub items
(no output)

# grep '^+.*def _' worker/worker_main.py
+def _real_startup_sequence() -> dict:
```

No new `pub` items introduced. The only new function is `_real_startup_sequence()` — private (leading underscore), matching the plan's Public API Surface table.

## Deviations from Plan

None. Implementation matches the approved plan exactly:
- `_real_startup_sequence()` reads all 4 env vars, calls `ipc.connect()`, imports torch inside the function, conditionally calls `torch.cuda.set_device()` for non-CPU, calls `capability.probe_capabilities()`, returns the dict.
- Google-style docstring with Args/Returns/Raises sections on `_real_startup_sequence()`.
- Inline `#` comments at decision points: env var reads, CPU vs GPU branch, torch import isolation.
- DEBUG log after `probe_capabilities()` returns, logging `device_type` and first 3 capability fields.
- `if __name__ == "__main__":` block calls `_real_startup_sequence()` and prints result, exits with code 0.
- 4 real-mode tests with env var isolation (save/restore in try/finally).
- No mock gate (`ANVILML_WORKER_MOCK` guard) exists in the file.
- `defers_to: P9-D2` — this task's `defers_to` field names P9-D2, but no stubs or mock implementations are written for deferred scope (the deferred items — node import, ready event, message dispatch loop — are not part of this task's implementation).

## Blockers

None.
