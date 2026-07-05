# Plan Report: P9-D1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P9-D1                                       |
| Phase       | 009 — Real Worker Startup                   |
| Description | worker_main.py: real-mode connect+device-select+probe (no mock gate) |
| Depends on  | P9-B1, P9-C2                                |
| Project     | anvilml                                     |
| Planned at  | 2026-07-05T14:55:00Z                        |
| Attempt     | 1                                           |

## Objective

Extend `worker/worker_main.py` with the first half of the real-mode startup sequence — connecting over IPC, importing torch, selecting the device, and running the real torch-level capability probe — establishing that no mock-only gate exists before any later step is added. When this task completes, a `python -m pytest worker/tests/test_worker_main.py -v -m real_mode` invocation passes with ≥3 real-mode tests, confirming the connect→device-select→probe sequence runs against a CPU device without raising and that `probe_capabilities()` is actually invoked (not skipped).

## Scope

### In Scope
- Add `main()` entry-point function in `worker/worker_main.py` that implements the real-mode startup sequence per ANVILML_DESIGN.md §14.2:
  1. Read `ANVILML_IPC_PORT` and `ANVILML_WORKER_ID` from environment; call `ipc.connect()`.
  2. Import `torch`; read `ANVILML_DEVICE_TYPE` and `ANVILML_DEVICE_INDEX`; call `torch.cuda.set_device(device_index)` for non-CPU, skip for CPU.
  3. Call `capability.probe_capabilities(device_type, device_index)` — the real torch-level probe from P9-C1.
  4. Return the capabilities dict.
- Add `if __name__ == "__main__":` block that calls `main()` and exits with the return code from `probe_capabilities()` (0 on success).
- Add ≥3 `@pytest.mark.real_mode` tests in `worker/tests/test_worker_main.py`:
  - `test_real_startup_calls_ipc_connect`: verifies `ipc.connect` is called with correct env var values.
  - `test_real_startup_selects_cpu_device`: verifies CPU device_type skips `torch.cuda.set_device`.
  - `test_real_startup_calls_probe_capabilities`: verifies `capability.probe_capabilities` is actually invoked and returns the expected 6-key bool dict.
  - `test_no_mock_gate_in_file`: verifies no `exit(1)` or `sys.exit(1)` guard against non-mock mode exists in the file.
- No mock gate (`if ANVILML_WORKER_MOCK != "1": exit(1)`) anywhere in the file.

### Out of Scope
- Node import (`_import_nodes()`) — deferred to P9-D2.
- Ready event send (`ipc.send_event(Ready{...})`) — deferred to P9-D2.
- Message dispatch loop — deferred to P9-D2.
- Mock-mode startup sequence — handled by P9-D3 (already has `_mock_probe_capabilities()` from P9-C2, but the full mock startup branch is P9-D3's scope).
- Integration test spawning a real subprocess — handled by P9-E1.

## Existing Codebase Assessment

`worker/worker_main.py` currently contains only `_mock_probe_capabilities()` (55 lines) — a pure-Python function returning fixed synthetic capability values, never importing torch. This was the deliverable of P9-C2.

`worker/ipc.py` (61 lines) is fully implemented per ANVILML_DESIGN.md §14.4, with `connect()`, `send_event()`, and `recv_message()` using ZeroMQ DEALER + msgpack. It is ready for use by the real-mode startup sequence.

`worker/capability.py` (171 lines) is fully implemented per ANVILML_DESIGN.md §6.6, with `probe_capabilities(device_type: str, device_index: int) -> dict` that constructs tiny `torch.nn.Linear` layers at each target dtype and runs forward passes. It is ready to be called from `worker_main.py`.

`worker/tests/test_worker_main.py` (107 lines) currently has only mock-mode tests (4 tests in `TestMockProbeCapabilities` and `TestNoTorchImport` classes). No real-mode tests exist yet.

Established patterns to follow:
- Google-style docstrings on all functions and classes.
- Inline `#` comments at decision points (device_type branch, env var reads).
- Subprocess isolation for torch-import checks (per ENVIRONMENT.md §11.3).
- Timeout on subprocess calls (`timeout=10`).
- The `real_mode` pytest marker is registered in `worker/pyproject.toml`.

Gap between design doc and current source: The design doc §14.2 describes the full startup sequence (IPC connect → torch import/device select → probe → node import → Ready event). Only the first three steps are in scope for this task. The existing `worker_main.py` has zero real-mode code — this task creates it from scratch.

## Resolved Dependencies

No new external packages are introduced by this task. It uses existing dependencies already declared in `worker/requirements/base.txt` and the CPU-specific requirement files:

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| python | pyzmq   | via base.txt    | pypi-query MCP | n/a                    |
| python | msgpack | via base.txt    | pypi-query MCP | n/a                    |
| python | torch   | via cpu-*.txt   | pypi-query MCP | n/a (CPU wheel only)   |

All three are already installed in the real-mode test environment. `os` and `sys` are Python stdlib.

## Approach

1. **Add `_real_startup_sequence()` function to `worker/worker_main.py`.**
   - This function implements the real-mode startup sequence per §14.2.
   - It reads env vars `ANVILML_IPC_PORT`, `ANVILML_WORKER_ID`, `ANVILML_DEVICE_TYPE`, `ANVILML_DEVICE_INDEX` from `os.environ`.
   - It calls `ipc.connect(int(os.environ["ANVILML_IPC_PORT"]), os.environ["ANVILML_WORKER_ID"])`.
   - It imports `torch` (import is inside the function, not at module level — this ensures torch is only imported during real-mode startup, not during mock-mode or test collection).
   - For non-CPU device types (`cuda`, `rocm`), it calls `torch.cuda.set_device(device_index)`. For `cpu`, it skips device selection entirely (the CPU device is implicit).
   - It calls `capability.probe_capabilities(device_type, device_index)` and returns the resulting dict.
   - The function has a Google-style docstring with Args/Returns/Raises sections.
   - Inline `#` comments at decision points: env var reads, the CPU vs GPU device selection branch.

2. **Add `if __name__ == "__main__":` block.**
   - Calls `_real_startup_sequence()` and prints the result.
   - Exits with code 0 on success.

3. **Add real-mode tests to `worker/tests/test_worker_main.py`.**
   - `test_real_startup_calls_ipc_connect`: Patches `ipc.connect` with `unittest.mock.patch`, calls `_real_startup_sequence()`, asserts `ipc.connect` was called with the correct port and worker_id from env vars. Sets env vars before calling, restores them after (per ENVIRONMENT.md §11.3).
   - `test_real_startup_cpu_skips_cuda_set_device`: Patches both `ipc.connect` and `capability.probe_capabilities`. Sets `ANVILML_DEVICE_TYPE=cpu`. Calls `_real_startup_sequence()`. Asserts `torch.cuda.set_device` was NOT called (because CPU skips device selection). Verifies `probe_capabilities` was called with `("cpu", 0)`.
   - `test_real_startup_calls_probe_capabilities`: Patches `ipc.connect`. Sets a non-CPU device type (e.g. `cuda`). Calls `_real_startup_sequence()`. Asserts `torch.cuda.set_device` WAS called with the correct device index. Asserts `capability.probe_capabilities` was called with the correct `(device_type, device_index)` args. Asserts the returned dict has exactly 6 keys, all bool.
   - `test_no_mock_gate_exit_path`: Reads the source file of `worker_main.py` as text, asserts no line matches the pattern `if.*ANVILML_WORKER_MOCK.*!=.*"1".*exit`. This is a mechanical check that the v3 defect pattern does not exist.

4. **Add logging.** Per §16.2/§16.3 of ANVILML_DESIGN.md, add a DEBUG log call after `probe_capabilities()` returns, logging the device_type and the first few capability fields.

## Public API Surface

| Item | Type | Module Path | Description |
|------|------|-------------|-------------|
| `_real_startup_sequence` | `def() -> dict` | `worker.worker_main` | Real-mode startup: connect IPC → import torch → select device → probe capabilities → return dict. |

No new `pub`/`def` at module level beyond the existing `_mock_probe_capabilities()`. The function is private (leading underscore), callable from tests via `import worker.worker_main as wm; wm._real_startup_sequence()`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/worker_main.py` | Add `_real_startup_sequence()` function and `if __name__ == "__main__":` block. |
| Modify | `worker/tests/test_worker_main.py` | Add ≥4 `@pytest.mark.real_mode` tests. |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_worker_main.py` | `test_real_startup_calls_ipc_connect` (real_mode) | `_real_startup_sequence()` calls `ipc.connect` with correct env var values for port and worker_id. | `ANVILML_IPC_PORT=5555`, `ANVILML_WORKER_ID=test-worker-0` set in env. `ipc.connect` patched. | Env vars set; function called. | `ipc.connect(5555, "test-worker-0")` called exactly once. | `python -m pytest worker/tests/test_worker_main.py -v -m real_mode -k test_real_startup_calls_ipc_connect` exits 0 |
| `worker/tests/test_worker_main.py` | `test_real_startup_cpu_skips_cuda_set_device` (real_mode) | CPU device_type skips `torch.cuda.set_device` entirely. | `ANVILML_DEVICE_TYPE=cpu`, `ANVILML_DEVICE_INDEX=0`. `ipc.connect` and `probe_capabilities` patched. | CPU device type; function called. | `torch.cuda.set_device` NOT called; `probe_capabilities("cpu", 0)` called. | `python -m pytest worker/tests/test_worker_main.py -v -m real_mode -k test_real_startup_cpu_skips_cuda_set_device` exits 0 |
| `worker/tests/test_worker_main.py` | `test_real_startup_calls_probe_capabilities` (real_mode) | Non-CPU device calls `torch.cuda.set_device` AND `capability.probe_capabilities` with correct args. | `ANVILML_DEVICE_TYPE=cuda`, `ANVILML_DEVICE_INDEX=1`. `ipc.connect` patched. | CUDA device type, index 1. | `torch.cuda.set_device(1)` called; `probe_capabilities("cuda", 1)` called; returns 6-key bool dict. | `python -m pytest worker/tests/test_worker_main.py -v -m real_mode -k test_real_startup_calls_probe_capabilities` exits 0 |
| `worker/tests/test_worker_main.py` | `test_no_mock_gate_exit_path` (real_mode) | No `if ANVILML_WORKER_MOCK != "1": exit(1)` gate exists anywhere in `worker_main.py`. | None. | Source file read as text. | No line matches the mock-gate pattern. | `python -m pytest worker/tests/test_worker_main.py -v -m real_mode -k test_no_mock_gate_exit_path` exits 0 |

## CI Impact

No CI changes required. This task only modifies existing Python files. The real-mode test suite (`-m real_mode`) is already configured to run in `worker-linux-real` and `worker-windows-real` CI jobs (ENVIRONMENT.md §6 Step 9). No new file types, no new gates.

## Platform Considerations

None identified. The device selection logic (`torch.cuda.set_device`) is called only for non-CPU device types (`cuda`, `rocm`), and real-mode tests run exclusively on CPU. The CPU path has no platform-specific code. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `torch.cuda.set_device()` on a CPU-only torch build may raise `RuntimeError` if called with any device index, even though the function should be a no-op on CPU builds. | Low | Medium | The function only calls `torch.cuda.set_device()` when `device_type != "cpu"`. For real-mode tests on CPU, we only test the `cpu` path (which skips `set_device` entirely). The non-CPU path is tested by patching `torch.cuda.set_device` to verify it would be called — we don't actually invoke it with a real CUDA device since none exists in CI. |
| `capability.probe_capabilities()` imports `torch` at module level, so calling it inside `_real_startup_sequence()` means torch is imported when the function runs. This could cause test failures if torch is not installed in the test environment. | Low | Medium | Real-mode tests (`-m real_mode`) run in environments where torch is installed (cpu-linux-agent.txt or cpu-runner-reqs.txt). The acceptance criterion explicitly requires `torch` to be importable for real-mode tests. We verify this by running the tests with `python -m pytest worker/tests/test_worker_main.py -v -m real_mode`, which only runs in environments with torch. |
| Mock-mode tests that import `worker_main` at collection time may transitively trigger the real-mode code path if `torch` is imported at module level. | Low | Medium | The `torch` import is inside `_real_startup_sequence()`, not at module level. Importing `worker_main` does not execute any function body. The existing `TestNoTorchImport` subprocess test (line 75-107) confirms this property is maintained. |

## Acceptance Criteria

- [ ] `python -m py_compile worker/worker_main.py` exits 0
- [ ] `python -m py_compile worker/tests/test_worker_main.py` exits 0
- [ ] `python -m pytest worker/tests/test_worker_main.py -v -m real_mode` exits 0 with ≥3 real-mode tests
- [ ] `grep -c 'if.*ANVILML_WORKER_MOCK.*!=.*"1".*exit' worker/worker_main.py` returns 0 (no mock gate)
- [ ] `grep -c 'def _real_startup_sequence' worker/worker_main.py` returns ≥1 (function exists)
- [ ] `grep -c 'capability.probe_capabilities' worker/worker_main.py` returns ≥1 (probe is called)
