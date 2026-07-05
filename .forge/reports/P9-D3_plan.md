# Plan Report: P9-D3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P9-D3                                       |
| Phase       | 009 — Real Worker Startup                   |
| Description | worker_main.py: mock-mode startup sequence  |
| Depends on  | P9-D2, P9-C2, P9-B1                         |
| Project     | anvilml                                     |
| Planned at  | 2026-07-05T17:10:00Z                        |
| Attempt     | 1                                           |

## Objective

Confirm that the mock-mode startup sequence in `worker/worker_main.py` is fully implemented per `ANVILML_DESIGN.md §14.3` — the `ANVILML_WORKER_MOCK=1` branch that connects IPC, probes synthetic capabilities (never importing torch), imports an empty node list, sends a `Ready` event with `capabilities_source="mock"`, and enters the dispatch loop. The acceptance criteria require >=4 unmarked tests verifying mock startup sends Ready with `capabilities_source="mock"` and that torch is never imported (subprocess-isolated check), with >=11 total tests across both modes in `test_worker_main.py`.

## Scope

### In Scope
- Verification that `_mock_startup_sequence()` exists and follows the §14.3 sequence: IPC connect → `_mock_probe_capabilities()` → `_import_nodes()` → `ipc.send_event(Ready{..., capabilities_source:"mock", node_types:[]})` → dispatch loop.
- Verification that `_mock_probe_capabilities()` returns a dict with 6 bool keys (`fp32`, `fp16`, `bf16`, `fp8`, `fp4`, `flash_attention`) and never imports torch.
- Verification that the `__main__` block dispatches to `_mock_startup_sequence()` when `ANVILML_WORKER_MOCK == "1"` and to `_real_startup_sequence()` otherwise, with the check happening once at the top-level entry point.
- Verification that >=4 tests exist in `worker/tests/test_worker_main.py` without markers covering: mock startup sends Ready with `capabilities_source="mock"`, torch is never imported (subprocess-isolated), mock probe returns correct keys/values, and the main block uses the correct dispatch pattern.
- Verification that >=11 total tests exist in the file across both modes.

### Out of Scope
- Real-mode startup sequence (P9-D1/P9-D2 scope).
- Node system implementation (Phase 10 scope).
- Integration test spawning a real subprocess (P9-E1 scope).
- CI job wiring (P9-F1 scope).
- Dual-mode parity markers on node/arch-module functions (Phase 10+ scope; not applicable to `worker_main.py`).

## Existing Codebase Assessment

**What exists:** The mock-mode startup sequence is already fully implemented in `worker/worker_main.py`:

1. `_mock_probe_capabilities()` (lines 208–249): Returns a dict with 6 bool keys matching `InferenceCaps` field names — `fp32`, `fp16`, `bf16`, `fp8` all True, `fp4` False, `flash_attention` True. Never imports torch.
2. `_mock_startup_sequence()` (lines 157–205): Reads env vars (`ANVILML_IPC_PORT`, `ANVILML_WORKER_ID`, `ANVILML_DEVICE_TYPE`, `ANVILML_DEVICE_INDEX`), calls `ipc.connect()`, calls `_mock_probe_capabilities()`, calls `_import_nodes()` (returns `[]`), sends `Ready` event with `capabilities_source="mock"`, enters `_dispatch_loop()`.
3. `__main__` block (lines 252–265): Checks `os.environ.get("ANVILML_WORKER_MOCK") == "1"` — dispatches to `_mock_startup_sequence()` if True, `_real_startup_sequence()` otherwise. The check happens once at the top-level entry point, not deep inside helpers.
4. `_import_nodes()` (lines 18–30): Returns `[]` — correct stub for Phase 9.
5. `_dispatch_loop()` (lines 33–63): Placeholder that logs received messages and exits on recv failure.

**Established patterns:**
- Tests use `unittest.mock.patch` to mock `ipc.connect`, `ipc.send_event`, and `ipc.recv_message` so startup sequences complete without a real IPC socket.
- Env var isolation: tests save all four startup env vars before mutating, restore unconditionally in a `finally` block.
- Subprocess isolation for torch-import checks (per §11.3 rule 7 — never use `sys.modules.pop`).
- Tests use `zmq.ZMQError("broken pipe")` as `recv_message` side_effect to break the dispatch loop cleanly.
- Google-style docstrings on all functions and test methods.
- Decision-point inline comments explaining why (e.g., why fp4 is False, why torch import is deferred inside the function body).

**Gap between design doc and current source:** None identified. The implementation matches `ANVILML_DESIGN.md §14.3` exactly. The sequence order, function names, and `Ready` event structure all match the spec.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| python | pyzmq   | (existing in venv) | No new dep  | n/a                    |
| python | msgpack | (existing in venv) | No new dep  | n/a                    |

No new external dependencies are introduced by this task. The mock-mode startup uses only stdlib (`os`, `sys`, `logging`) and the already-existing `ipc.py` module.

## Approach

**defers_to (from JSON): []** — This task implements its full scope. No deferrals.

The mock-mode startup sequence is already implemented in `worker_main.py`. This task's work is verification: confirm the implementation matches §14.3, confirm the test suite meets the acceptance criteria counts, and confirm no defects exist.

1. **Verify `_mock_startup_sequence()` structure.** Confirm it follows the exact §14.3 sequence: read env vars → `ipc.connect(port, worker_id)` → `_mock_probe_capabilities()` → `_import_nodes()` → `ipc.send_event(Ready{..., capabilities_source="mock", node_types:[]})` → `_dispatch_loop()`. Each step is already present in lines 157–205 of `worker_main.py`.

2. **Verify `_mock_probe_capabilities()` correctness.** Confirm it returns a dict with exactly 6 keys matching `InferenceCaps` field names, all bool values, `fp4=False`, others `True`. Already implemented at lines 208–249.

3. **Verify `__main__` dispatch logic.** Confirm the check `os.environ.get("ANVILML_WORKER_MOCK") == "1"` appears once at the top-level entry point (line 260), dispatching to `_mock_startup_sequence()` or `_real_startup_sequence()` respectively. No env-var re-checking inside helper functions.

4. **Verify test counts.** Count all `def test_` methods in `worker/tests/test_worker_main.py`:
   - Unmarked tests (run in both modes): 4+ (TestMockProbeCapabilities × 3, TestNoTorchImport × 1, plus module-level mock startup and gate tests).
   - `@pytest.mark.real_mode` tests: 8 (TestRealStartupSequence × 5, TestNoMockGate × 1, plus module-level real-mode tests).
   - Total: >=12, exceeding the >=11 requirement.

5. **Verify torch-isolation test.** Confirm `test_no_torch_import_on_module_load` uses subprocess isolation (not `sys.modules.pop`) with `timeout=10` per ENVIRONMENT.md §11.3.

6. **Verify no mock-only gate exists.** Confirm no line in `worker_main.py` matches the v3 defect pattern: `if ANVILML_WORKER_MOCK != "1": exit(1)`. The existing `test_no_mock_gate_in_main_block` test performs this mechanical check.

7. **Confirm dual-mode parity markers do not apply.** The §10.6 marker convention applies to node `execute()` and arch module `load()`/`sample()`/`decode()` functions. `worker_main.py` has none of these — its functions are startup-sequence helpers (`_mock_startup_sequence`, `_real_startup_sequence`, `_mock_probe_capabilities`). No markers needed.

8. **Confirm phase-closing audit does not apply.** P9-D3 is not the last task in `tasks_phase009.json` — P9-E1 (integration test) and P9-F1 (CI wiring) follow it. §9a audit is not required.

## Public API Surface

No new public API items are introduced. All functions are module-private (prefixed with `_`):

- `_mock_startup_sequence() -> None` — mock-mode entry point (already exists)
- `_mock_probe_capabilities() -> dict` — synthetic capability values (already exists)

Existing public module-level items referenced:
- `_import_nodes() -> list` — returns `[]` (already exists)
- `_dispatch_loop() -> None` — message loop placeholder (already exists)

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Verify | `worker/worker_main.py` | Mock-mode startup sequence already implemented; verify correctness |
| Verify | `worker/tests/test_worker_main.py` | Test suite already present; verify test counts meet acceptance criteria |

No files are created or modified. This task is a verification/confirmation task — the mock-mode startup sequence was already implemented in prior tasks (P9-C2 and the P9-D1/P9-D2 refactoring that merged both branches).

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|---------------------|
| `worker/tests/test_worker_main.py` | `test_returns_six_required_keys` | `_mock_probe_capabilities()` returns dict with exactly 6 required keys | `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestMockProbeCapabilities::test_returns_six_required_keys -v` exits 0 |
| `worker/tests/test_worker_main.py` | `test_all_values_are_bool` | All 6 values in returned dict are `bool` type | `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestMockProbeCapabilities::test_all_values_are_bool -v` exits 0 |
| `worker/tests/test_worker_main.py` | `test_fp4_is_false` | `fp4` key specifically maps to `False` | `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestMockProbeCapabilities::test_fp4_is_false -v` exits 0 |
| `worker/tests/test_worker_main.py` | `test_no_torch_import_on_module_load` | Importing `worker_main` does not transitively import torch (subprocess-isolated) | `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestNoTorchImport::test_no_torch_import_on_module_load -v` exits 0 |
| `worker/tests/test_worker_main.py` | `test_mock_startup_sends_ready_event` | `_mock_startup_sequence()` sends Ready event with `capabilities_source="mock"` (unmarked, runs in both modes) | `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::test_mock_startup_sends_ready_event -v` exits 0 |
| `worker/tests/test_worker_main.py` | `test_no_mock_gate_in_main_block` | `__main__` block uses `== "1"` check, not `!= "1"` exit pattern | `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::test_no_mock_gate_in_main_block -v` exits 0 |

These 6 unmarked tests cover the mock startup acceptance criteria. The remaining 8 `@pytest.mark.real_mode` tests cover the real-mode path. Total: 14 tests >= 11.

## CI Impact

No CI changes required. The test file already exists and is collected by the existing `worker-linux-mock` and `worker-windows-mock` CI jobs. The unmarked tests run in both mock and real CI jobs; the `@pytest.mark.real_mode` tests are excluded from mock-mode CI via `-m "not real_mode"`.

## Platform Considerations

None identified. The mock-mode startup sequence is platform-neutral:
- `_mock_probe_capabilities()` returns a static dict — no platform-specific code.
- `ipc.connect()` uses `tcp://127.0.0.1:{port}` which is identical on Linux and Windows.
- `_dispatch_loop()` uses the same `while True` loop on all platforms.
- The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Existing tests may have been written for a slightly different version of `_mock_startup_sequence()` (e.g., different env var names or a different `Ready` event structure) that was later changed in `worker_main.py`. | Low | Medium | Verify test assertions against the actual source code in `worker_main.py`. The plan step 1 confirms each assertion matches the current implementation. |
| The subprocess-isolated torch-import test (`test_no_torch_import_on_module_load`) may fail in an environment where `torch` is already installed at the system level and the subprocess inherits it from the parent. | Low | Medium | The subprocess is spawned with `subprocess.run([sys.executable, "-c", ...])` which uses a fresh Python interpreter. If the venv has torch installed, this test would fail — but that's correct behavior for real-mode CI. The test is designed to verify mock-mode isolation, and the mock CI job installs only `base.txt` (no torch), so it passes there. |
| A future task adding node types to `_import_nodes()` may cause the mock startup's `node_types=[]` assertion to fail. | Low | Low | When Phase 10 implements real node import, the `test_mock_startup_sends_ready_event` test will need updating to accept a non-empty `node_types` list. This is a known future change. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py -v --collect-only 2>&1 | grep -c "test session starts\|test_"` shows >=11 tests (total across both modes)
- [ ] `grep -c "def test_" /home/dryw/AnvilML/worker/tests/test_worker_main.py` returns >=11
- [ ] `grep -c "@pytest.mark.real_mode" /home/dryw/AnvilML/worker/tests/test_worker_main.py` returns >=3 (real-mode tests exist)
- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestMockProbeCapabilities -v` exits 0 (mock probe tests pass)
- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py::TestNoTorchImport -v` exits 0 (torch isolation test passes)
- [ ] `grep -c "_mock_startup_sequence" /home/dryw/AnvilML/worker/worker_main.py` returns >=1 (function exists)
- [ ] `grep -c "_mock_probe_capabilities" /home/dryw/AnvilML/worker/worker_main.py` returns >=1 (function exists)
- [ ] `grep "ANVILML_WORKER_MOCK.*==.*\"1\"" /home/dryw/AnvilML/worker/worker_main.py` matches exactly one dispatch line (single check at entry point)
- [ ] `grep -c "capabilities_source.*mock" /home/dryw/AnvilML/worker/worker_main.py` returns >=1 (Ready event carries "mock")
