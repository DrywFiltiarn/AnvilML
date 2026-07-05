# Plan Report: P9-C2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P9-C2                                       |
| Phase       | 009 — Real Worker Startup                   |
| Description | worker_main.py: _mock_probe_capabilities() synthetic values |
| Depends on  | P9-C1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-07-05T14:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `worker/worker_main.py` containing the single function `def _mock_probe_capabilities() -> dict` that returns fixed synthetic capability values (all True except `fp4`) matching the `InferenceCaps` struct field names. This function is the mock-mode equivalent of the real torch-level probe in `capability.py` — it never imports `torch`, enabling mock-mode CI jobs to run without any GPU driver or torch installation. Three acceptance tests go in `worker/tests/test_worker_main.py`.

## Scope

### In Scope
- Create `worker/worker_main.py` with module docstring and `def _mock_probe_capabilities() -> dict` returning a dict with keys `fp32`, `fp16`, `bf16`, `fp8`, `fp4`, `flash_attention` (all `bool`), where all are `True` except `fp4` which is `False` (per `ANVILML_DESIGN.md §14.3` synthetic values).
- Create `worker/tests/test_worker_main.py` with ≥3 tests: (1) function returns the 6 required keys, (2) all values are `bool`, (3) subprocess isolation confirms `torch` was never imported as a side effect of importing `worker_main`.
- No other functions, no startup sequence, no IPC logic — those are later tasks (P9-D1, P9-D2, P9-D3).

### Out of Scope
None. This task's `defers_to` field is `[]` (empty). All functionality described in the task context is implemented in full. No stubs, no placeholders, no deferred scope.

## Existing Codebase Assessment

The `worker/` directory already exists with `ipc.py`, `capability.py`, `pyproject.toml`, and test files (`test_ipc.py`, `test_capability.py`, `conftest.py`). `worker_main.py` does not exist yet — this task creates it from scratch.

The established patterns are clear from the existing code:
- **Module docstrings**: Google-style, one-sentence summary followed by a longer description. `capability.py`'s docstring explicitly notes that the mock equivalent lives inline in `worker_main.py` and never imports torch.
- **Test style**: Tests use `class TestXxx:` grouping with descriptive method names (`test_abc_xyz_returns_true`). Docstrings on every test describe what it verifies, preconditions, and expected outcome.
- **No-torch isolation test**: `test_ipc.py` has `TestNoTorchImport.test_module_no_torch_import()` which spawns a subprocess via `subprocess.run()` with `timeout=10`, asserts `"torch" not in sys.modules`, and checks `returncode == 0`. This is the exact pattern to replicate.
- **No pytest markers needed**: Tests without `@pytest.mark.real_mode` are assumed mock-compatible and run in both mock and real CI jobs. These tests must not import torch at module level.
- **The `InferenceCaps` struct** (in `anvilml-core/src/types/hardware.rs`) defines exactly 6 fields: `fp32`, `fp16`, `bf16`, `fp8`, `fp4`, `flash_attention`. The mock probe returns a dict with matching keys.

The design doc (`ANVILML_DESIGN.md §14.3`) specifies the mock-mode sequence differs from real-mode in exactly one step (capability probing). The mock probe returns fixed synthetic values — all True except `fp4`. This is a deliberate design choice: `fp4` has no native torch dtype support on any current build, so it is correctly always False.

## Resolved Dependencies

None. This task introduces no new external dependencies. `_mock_probe_capabilities()` is a pure Python function returning a static dict — no imports beyond the standard library (none needed, since the dict is hardcoded).

## Approach

1. **Create `worker/worker_main.py`** with:
   - Module-level docstring (Google style) describing the file's purpose: entry point for the Python worker process, and noting that real-mode startup (torch import, real probe) is the default branch while mock mode is the explicit alternate.
   - The function `def _mock_probe_capabilities() -> dict:` with a Google-style docstring explaining it returns fixed synthetic capability values matching `InferenceCaps` field names, never imports torch, and is the mock-mode equivalent of `capability.probe_capabilities()`.
   - The function body returns the dict literal:
     ```python
     return {
         "fp32": True,
         "fp16": True,
         "bf16": True,
         "fp8": True,
         "fp4": False,
         "flash_attention": True,
     }
     ```
   - Rationale for `fp4=False`: Torch 2.x has no native `torch.float4` dtype. The real probe in `capability.py` attempts `torch.float8_e4m3fn` for both `fp8` and `fp4` (line 143), and on CPU that raises `NotImplementedError`. Since `fp4` is universally unsupported on current torch builds, the mock correctly returns `False`.
   - Rationale for `fp8=True`: While the real probe returns `fp8=False` on CPU (because `torch.float8_e4m3fn` raises on CPU), the mock-mode synthetic values are device-agnostic defaults — they represent what a GPU-capable device would report. The mock is a synthetic baseline, not a CPU simulation. (This matches the pattern: mock returns "all capable except fp4".)
   - Rationale for `flash_attention=True`: On modern torch builds, `scaled_dot_product_attention` is always available (at minimum via the math fallback), so this is correctly `True` for the synthetic baseline.

2. **Create `worker/tests/test_worker_main.py`** with:
   - Module docstring describing the test file's purpose.
   - Test class `TestMockProbeCapabilities` with:
     - `test_returns_six_required_keys()`: Imports `_mock_probe_capabilities`, calls it, asserts the result has exactly the 6 expected keys matching `InferenceCaps` field names.
     - `test_all_values_are_bool()`: Iterates over all returned values, asserts each is `isinstance(value, bool)`.
   - Test class `TestNoTorchImport` with:
     - `test_no_torch_import_on_module_load()`: Spawns a fresh subprocess via `subprocess.run([sys.executable, "-c", "..."])` that imports `worker.worker_main` and asserts `"torch" not in sys.modules`. Uses `timeout=10` per `ENVIRONMENT.md §11.3`. Checks `returncode == 0`.

3. **No `__main__` block**: The file contains only the function — no `if __name__ == "__main__"` block, no startup logic. Those are added in later tasks (P9-D1, P9-D3).

## Public API Surface

| Item | Module Path | Signature |
|------|-------------|-----------|
| `_mock_probe_capabilities` | `worker.worker_main` | `def _mock_probe_capabilities() -> dict` |

This is a private function (prefixed with `_`), not `pub`/`def public`. It is tested via direct import in the test file.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/worker_main.py` | Module with `_mock_probe_capabilities()` function and module docstring |
| CREATE | `worker/tests/test_worker_main.py` | Test file with ≥3 tests for the mock probe function |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_worker_main.py` | `test_returns_six_required_keys` | `_mock_probe_capabilities()` returns a dict with exactly the 6 keys matching `InferenceCaps` field names (`fp32`, `fp16`, `bf16`, `fp8`, `fp4`, `flash_attention`) | None (pure function, no setup) | None (no args) | Dict with exactly 6 keys, no more, no fewer | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py::TestMockProbeCapabilities::test_returns_six_required_keys -v` exits 0 |
| `worker/tests/test_worker_main.py` | `test_all_values_are_bool` | All 6 values in the returned dict are `bool` type (not `int`, `str`, or other) | None | None | Every `isinstance(v, bool)` is True | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py::TestMockProbeCapabilities::test_all_values_are_bool -v` exits 0 |
| `worker/tests/test_worker_main.py` | `test_no_torch_import_on_module_load` | Importing `worker.worker_main` does not transitively import `torch` (confirmed via subprocess isolation) | None | Subprocess runs `import worker.worker_main; import sys; assert 'torch' not in sys.modules` | Subprocess exit code 0, stdout contains "OK" | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py::TestNoTorchImport::test_no_torch_import_on_module_load -v` exits 0 |
| `worker/tests/test_worker_main.py` | `test_fp4_is_false` | The `fp4` key specifically maps to `False` (the one deliberate exception in synthetic values) | None | None | `result["fp4"] is False` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py::TestMockProbeCapabilities::test_fp4_is_false -v` exits 0 |

## CI Impact

No CI changes required. The new test file follows the established naming convention (`test_*.py` in `worker/tests/`) and is automatically picked up by the existing `worker-linux-mock` and `worker-windows-mock` CI jobs (which run `pytest worker/tests -v -m "not real_mode"`). These jobs install only `base.txt` (no torch), which is exactly what this task's code requires.

## Platform Considerations

None identified. The function returns a pure Python dict literal with no platform-specific behaviour. The subprocess test uses `sys.executable` (from the venv) which works identically on Linux and Windows.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ANVILML_WORKER_MOCK=1` env var not set in subprocess test, causing the subprocess to attempt real-mode startup (which would fail without torch) | Low | Medium | The subprocess test only imports `worker.worker_main` and checks `sys.modules` — it does not call `_mock_probe_capabilities()` nor does it set any env var. The function itself is unconditional and never reads env vars. No risk of accidental real-mode path execution. |
| The subprocess timeout of 10s is too generous for a pure import check | Low | Low | `subprocess.run` with `timeout=10` is the established pattern from `test_ipc.py`. A 10s timeout on an import that takes milliseconds is harmless and provides safety against hung processes. |
| `fp4` value mismatch between mock and real probe semantics | Low | Low | The real probe on CPU returns `fp4=False` (torch has no native fp4 dtype). The mock also returns `fp4=False`. They agree on this value. The mock's other values (`fp8=True`) differ from CPU real-mode (`fp8=False`), but that is correct — the mock represents a GPU-capable device baseline, not a CPU simulation. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/worker_main.py` exits 0
- [ ] `worker/.venv/bin/python -m py_compile worker/tests/test_worker_main.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py -v` exits 0 (all ≥3 tests pass)
- [ ] `python3.12 -c "import sys; sys.path.insert(0, '.'); import worker.worker_main; assert 'torch' not in sys.modules; print('OK')"` exits 0 (torch was never imported by importing worker_main)
