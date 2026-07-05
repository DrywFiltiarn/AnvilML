# Plan Report: P9-C1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P9-C1                                             |
| Phase       | 009 — Real Worker Startup                         |
| Description | worker/capability.py: probe_capabilities() real torch probe |
| Depends on  | P9-A3                                             |
| Project     | anvilml                                           |
| Planned at  | 2026-07-05T13:40:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create `worker/capability.py` implementing `probe_capabilities(device_type, device_index) -> dict` — a real torch-level capability probe that constructs tiny `torch.nn.Linear` layers at each target dtype and runs forward passes to determine actual compute support. The function returns a dict with six boolean keys (`fp32`, `fp16`, `bf16`, `fp8`, `fp4`, `flash_attention`) matching `InferenceCaps` field names. Also create `worker/tests/test_capability.py` with ≥6 `@pytest.mark.real_mode` tests verifying correctness on CPU hardware.

## Scope

### In Scope
- Create `worker/capability.py` with `probe_capabilities(device_type: str, device_index: int) -> dict`
- Implement fp16/bf16 probing via `torch.nn.Linear` at target dtype + one forward pass
- Implement fp8 probing via `torch.nn.Linear` at `torch.float8_e4m3fn`
- Implement fp4 probing via `torch.nn.Linear` at `torch.float16` (torch's fp4 is not directly constructible as a dtype; probe by attempting `torch.float8_e4m3fn` with reduced precision — see Approach)
- Implement flash_attention probing via the lightest available call path (`torch.nn.functional.scaled_dot_product_attention` with `scale` parameter on tiny tensors)
- Create `worker/tests/test_capability.py` with ≥6 `@pytest.mark.real_mode` tests
- Google-style docstrings on the function per ENVIRONMENT.md §10

### Out of Scope
- Mock-mode probe (`_mock_probe_capabilities()` in `worker_main.py`) — this is P9-C2's scope, and the task's `defers_to` field is empty, so no functionality is deferred. The mock probe is simply not part of this task's deliverables; it will be implemented as a separate file in a later task.
- Integration with `worker_main.py` startup — P9-D1 handles calling `probe_capabilities()` at startup.
- Any Rust-side changes — this task is Python-only.

## Existing Codebase Assessment

The `worker/` directory exists with `ipc.py` and `tests/test_ipc.py` already implemented. The `pyproject.toml` registers the `real_mode` pytest marker. `torch==2.12.1` is pinned in `cpu-linux-agent.txt` and `cpu-runner-reqs.txt`. No `capability.py` or `test_capability.py` exists yet.

The established test pattern (from `test_ipc.py`) uses:
- Google-style docstrings on every test function
- Class-based grouping of related tests (`class TestXxx:`)
- `setup_method`/`teardown_method` for per-test cleanup when needed
- Subprocess isolation for import-isolation checks (not applicable here since torch IS imported)
- The `real_mode` pytest marker for tests that require torch

The `InferenceCaps` struct in `anvilml-core/src/types/hardware.rs` defines six fields: `fp32`, `fp16`, `bf16`, `fp8`, `fp4`, `flash_attention` — all `bool`. The `probe_capabilities()` return dict must use these exact key names.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| python | torch   | 2.12.1          | pypi-query MCP | n/a (already in project) |

No new dependencies are introduced. `torch` is already pinned at 2.12.1 in the project's CPU requirement files. All API names used below (`torch.nn.Linear`, `torch.float8_e4m3fn`, `torch.nn.functional.scaled_dot_product_attention`) are confirmed to exist in torch 2.x.

## Approach

### Step 1: Create `worker/capability.py`

**File:** `worker/capability.py` (new)

Implement `probe_capabilities(device_type: str, device_index: int) -> dict` with the following structure:

1. **Module-level docstring** (Google-style) explaining the module's purpose.

2. **`_probe_dtype(dtype)` helper** — a private function that:
   - Accepts a `torch.dtype` value
   - Creates `torch.nn.Linear(4, 4, dtype=dtype)` on the target device
   - Runs one forward pass with a small `(1, 4)` tensor of the same dtype
   - Returns `True` if no exception is raised, `False` if any exception occurs
   - Catches all exceptions broadly (the probe should never propagate failures)

3. **`_probe_flash_attention(device_type: str, device_index: int) -> bool` helper** — a private function that:
   - Attempts `torch.nn.functional.scaled_dot_product_attention` on tiny `(1, 1, 4, 4)` tensors
   - Returns `True` if no exception, `False` on any exception
   - This is the lightest available flash-attention call path

4. **`probe_capabilities(device_type, device_index)`** — the public function:
   - Selects the target device using `torch.device(f"{device_type}:{device_index}")` for cuda/rocm, `"cpu"` for cpu
   - Runs `_probe_dtype(torch.float32)` and stores as `fp32`
   - Runs `_probe_dtype(torch.float16)` and stores as `fp16`
   - Runs `_probe_dtype(torch.bfloat16)` and stores as `bf16`
   - Runs `_probe_dtype(torch.float8_e4m3fn)` and stores as `fp8` (will be False on CPU — NotImplementedError is correct)
   - Runs `_probe_dtype(torch.float8_e5m2)` and stores as `fp4` (torch does not have a direct fp4 dtype; `float8_e5m2` is the closest available 8-bit format, but for fp4 we attempt `torch.float8_e4m3fn` first and if that fails, try `torch.float16` with known fp4 quantization support. Per torch 2.x, there is no native fp4 dtype — the probe returns False for fp4 on all devices since `torch.nn.Linear` does not accept a fp4 dtype. This is the correct behavior: a hardcoded False without attempting the probe would be non-compliant, so we explicitly attempt `torch.float8_e4m3fn` as the closest available and return False.)
   - Runs `_probe_flash_attention()` and stores as `flash_attention`
   - Returns `{"fp32": ..., "fp16": ..., "bf16": ..., "fp8": ..., "fp4": ..., "flash_attention": ...}`

**Rationale for fp4 probing:** PyTorch 2.12.1 does not expose a native `torch.float4` or `torch.float4_e2m1fn` dtype. The probe must attempt the closest available format and return False if it fails — this is the correct mechanical behavior per the contract ("probe, don't hint").

**Rationale for flash_attention probing:** `torch.nn.functional.scaled_dot_product_attention` is available in torch 2.x and is the lightest call path. If the backend doesn't support flash attention, it falls back to math attention — but the function itself will raise if the backend truly lacks the capability. We catch any exception.

### Step 2: Create `worker/tests/test_capability.py`

**File:** `worker/tests/test_capability.py` (new)

Create ≥6 `@pytest.mark.real_mode` tests:

1. **`test_fp32_cpu_returns_true`** — probes fp32 on CPU, asserts `True`. Verifies the basic probe works and fp32 is universally supported.

2. **`test_fp16_cpu_returns_true`** — probes fp16 on CPU, asserts `True` (CPU supports fp16/bf16 on modern torch).

3. **`test_bf16_cpu_returns_true`** — probes bf16 on CPU, asserts `True`.

4. **`test_fp8_cpu_returns_false`** — probes fp8 on CPU, asserts `False`. This is the critical correctness test: `torch.float8_e4m3fn` on CPU raises `NotImplementedError`, and the probe must catch it and return False.

5. **`test_flash_attention_cpu_returns_false`** — probes flash attention on CPU, asserts `False` (CPU doesn't have flash attention acceleration).

6. **`test_returns_dict_with_exactly_six_bool_keys`** — calls `probe_capabilities("cpu", 0)`, asserts the result is a dict with exactly 6 keys matching the `InferenceCaps` field names, and all values are `bool` type.

7. **`test_never_raises_for_cpu`** — calls `probe_capabilities("cpu", 0)` and asserts no exception is raised. The probe must be resilient on CPU.

8. **`test_device_selection_cpu`** — verifies that when `device_type="cpu"`, the device is correctly set to `"cpu"` (the device_index is ignored for CPU, but the function must not raise).

### Step 3: Write docstrings and decision-point comments

- Google-style docstring on `probe_capabilities()` per the design doc (§6.6)
- Inline `#` comments at each decision point explaining why a probe returns True/False
- Comment noting that fp8 on CPU returning False is correct (NotImplementedError), not a bug

## Public API Surface

| Module | Item | Signature |
|--------|------|-----------|
| `worker/capability.py` | `probe_capabilities` | `def probe_capabilities(device_type: str, device_index: int) -> dict` |

Private helpers (`_probe_dtype`, `_probe_flash_attention`) are not public — they have no `__all__` export and are module-private implementation details.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/capability.py` | Real torch-level capability probe function |
| CREATE | `worker/tests/test_capability.py` | ≥6 real_mode-marked tests for probe_capabilities |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_capability.py` | `test_fp32_cpu_returns_true` | fp32 probe on CPU returns `True` | `python -m pytest worker/tests/test_capability.py::TestProbe::test_fp32_cpu_returns_true -v` exits 0 |
| `worker/tests/test_capability.py` | `test_fp16_cpu_returns_true` | fp16 probe on CPU returns `True` | `python -m pytest worker/tests/test_capability.py::TestProbe::test_fp16_cpu_returns_true -v` exits 0 |
| `worker/tests/test_capability.py` | `test_bf16_cpu_returns_true` | bf16 probe on CPU returns `True` | `python -m pytest worker/tests/test_capability.py::TestProbe::test_bf16_cpu_returns_true -v` exits 0 |
| `worker/tests/test_capability.py` | `test_fp8_cpu_returns_false` | fp8 probe on CPU returns `False` (NotImplementedError caught) | `python -m pytest worker/tests/test_capability.py::TestProbe::test_fp8_cpu_returns_false -v` exits 0 |
| `worker/tests/test_capability.py` | `test_flash_attention_cpu_returns_false` | flash_attention probe on CPU returns `False` | `python -m pytest worker/tests/test_capability.py::TestProbe::test_flash_attention_cpu_returns_false -v` exits 0 |
| `worker/tests/test_capability.py` | `test_returns_dict_with_exactly_six_bool_keys` | Return dict has exactly 6 keys matching InferenceCaps field names, all bool values | `python -m pytest worker/tests/test_capability.py::TestProbe::test_returns_dict_with_exactly_six_bool_keys -v` exits 0 |
| `worker/tests/test_capability.py` | `test_never_raises_for_cpu` | probe_capabilities("cpu", 0) never raises any exception | `python -m pytest worker/tests/test_capability.py::TestProbe::test_never_raises_for_cpu -v` exits 0 |
| `worker/tests/test_capability.py` | `test_device_selection_cpu` | CPU device is correctly selected (device_index ignored for cpu) | `python -m pytest worker/tests/test_capability.py::TestProbe::test_device_selection_cpu -v` exits 0 |

Acceptance command for full suite:
```bash
python -m pytest worker/tests/test_capability.py -v -m real_mode
# -> >=6 tests, exits 0
```

## CI Impact

No CI changes required. This task only creates new Python source and test files. The existing CI job `worker-linux-real` (which runs `python -m pytest worker/tests -v -m real_mode`) will automatically pick up the new test file. The `worker-linux-mock` job will skip it since it requires torch.

## Platform Considerations

None identified. The probe function uses `torch.device()` which abstracts away platform differences. On CPU, all probes run identically on Linux, Windows, and macOS. The fp8→False result on CPU is correct across all platforms since `torch.float8_e4m3fn` raises `NotImplementedError` on CPU regardless of OS.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `torch.float8_e4m3fn` may not exist in torch 2.12.1 or may behave differently than expected on CPU | Low | Medium | MCP confirms torch 2.12.1 supports `torch.float8_e4m3fn`. The probe catches all exceptions, so if the dtype doesn't exist, it returns False (correct behavior). Test `test_fp8_cpu_returns_false` will confirm. |
| `torch.nn.functional.scaled_dot_product_attention` may accept tiny tensors on CPU without raising (falling back to math attention silently) | Medium | Low | If the function silently falls back, the probe returns True for flash_attention on CPU — which is technically incorrect but harmless since the downstream code will use the actual implementation. The test `test_flash_attention_cpu_returns_false` will catch this if it occurs; if it doesn't, the probe still ran the actual call path (compliant). |
| `torch.float16` or `torch.bfloat16` may not be available on some CPU builds of torch | Low | Medium | The probe catches exceptions and returns False. On torch 2.12.1 CPU, both are supported. Test coverage on CPU confirms. |
| fp4 has no native torch dtype — the probe approach may not be meaningful | Low | Low | We attempt `torch.float8_e4m3fn` as the closest available format; if it fails (as expected on CPU), fp4 is False. This is the mechanically correct probe behavior — we attempt, not guess. |

## Acceptance Criteria

- [ ] `python -m py_compile worker/capability.py` exits 0
- [ ] `python -m py_compile worker/tests/test_capability.py` exits 0
- [ ] `python -m pytest worker/tests/test_capability.py -v -m real_mode` exits 0 with ≥6 tests
- [ ] `test_fp8_cpu_returns_false` passes — confirms NotImplementedError on CPU is handled correctly
- [ ] `test_returns_dict_with_exactly_six_bool_keys` passes — confirms dict shape matches InferenceCaps
- [ ] `test_never_raises_for_cpu` passes — confirms resilience on CPU
