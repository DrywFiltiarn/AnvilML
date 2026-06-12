# Plan Report: P21-A4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P21-A4                                      |
| Phase       | 021 — Real Python Worker — ZiT              |
| Description | worker: defaults.py + requirements (cuda/rocm/cpu) populated |
| Depends on  | P21-A3                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-06-12T19:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `worker/defaults.py` with the `ModelDefaults` dataclass and per-model default instances (`ZIT_DEFAULTS`, `SDXL_DEFAULTS`), and populate the worker requirements files (`base.txt`, `cuda.txt`, `rocm-linux.txt`, `rocm-windows.txt`, `cpu.txt`) with correct version constraints and torch selectors for each OS/backend combination.

## Scope

### In Scope
- Create `worker/defaults.py`:
  - `ModelDefaults` dataclass (or namedtuple) with fields: `steps`, `guidance_scale`, `width`, `height`, `dtype`, plus optional `supports_negative_prompt`
  - `ZIT_DEFAULTS = ModelDefaults(steps=8, guidance_scale=0.0, width=1024, height=1024, dtype="bf16")`
  - `SDXL_DEFAULTS = ModelDefaults(steps=20, guidance_scale=7.5, width=1024, height=1024, dtype="fp16", supports_negative_prompt=True)`
- Populate `worker/requirements/base.txt` with pinned minimum versions:
  - `diffusers>=0.27`
  - `transformers>=4.40`
  - `accelerate` (no minimum — pulled in by diffusers/transformers)
  - `Pillow>=10.0`
  - `msgpack>=1.0`
  - `numpy`
  - `safetensors`
  - `pytest`
- Ensure torch selector files are properly populated:
  - `worker/requirements/cuda.txt` — `--index-url https://download.pytorch.org/whl/cu124` + `torch>=2.5.0`
  - `worker/requirements/rocm-linux.txt` — `--index-url https://download.pytorch.org/whl/rocm6.2` + `torch>=2.5.0`
  - `worker/requirements/rocm-windows.txt` — AMD PyTorch-on-Windows index + `torch>=2.5.0`
  - `worker/requirements/cpu.txt` — `torch>=2.5.0` (CPU-only)
- Create `worker/tests/test_defaults.py` that imports `defaults` and verifies the default objects exist with correct field values (no GPU/torch required).

### Out of Scope
- Any ZiT or SDXL node implementation (P21-A5)
- Executor implementation (P21-A2)
- Pipeline cache implementation (P21-A3)
- Rust/Python KNOWN_NODE_TYPES parity test (P21-A6)
- Runnable proof documentation (P21-A7)
- Any Rust code changes
- CI workflow modifications

## Approach

1. **Verify prerequisites**: Confirm `worker/defaults.py` does not exist yet (confirmed via glob — it is absent). Confirm `P21-A3` (pipeline_cache.py) exists (confirmed).

2. **Create `worker/defaults.py`**:
   - Import `dataclasses` (or `typing.NamedTuple`).
   - Define `ModelDefaults` as a `@dataclass` with fields: `steps: int`, `guidance_scale: float`, `width: int`, `height: int`, `dtype: str`, and `supports_negative_prompt: bool = False`.
   - Create `ZIT_DEFAULTS = ModelDefaults(steps=8, guidance_scale=0.0, width=1024, height=1024, dtype="bf16")`.
   - Create `SDXL_DEFAULTS = ModelDefaults(steps=20, guidance_scale=7.5, width=1024, height=1024, dtype="fp16", supports_negative_prompt=True)`.
   - No logging required (pure data, no I/O, no decision points — §11.1 exception).

3. **Populate `worker/requirements/base.txt`**:
   - Replace existing content with the full list including version constraints:
     ```
     diffusers>=0.27
     transformers>=4.40
     accelerate
     Pillow>=10.0
     msgpack>=1.0
     numpy
     safetensors
     pytest
     ```
   - The existing file already has most of these but is missing `accelerate` and has no version constraint on `diffusers` or `transformers`.

4. **Verify torch selector files** (design §21.3):
   - `cuda.txt`: Already has `--index-url https://download.pytorch.org/whl/cu124` + `torch>=2.5.0`. Verified correct.
   - `rocm-linux.txt`: Already has `--index-url https://download.pytorch.org/whl/rocm6.2` + `torch>=2.5.0`. Verified correct.
   - `rocm-windows.txt`: Currently uses a Tsinghua mirror fallback which is not the standard AMD PyTorch-on-Windows index. Per design §21.3, this should reference AMD's hosted wheels for ROCm ≥ 7.2. Update to use the correct AMD PyTorch-on-Windows extra-index-url: `--extra-index-url https://download.pytorch.org/whl/rocm6.2` (or AMD's official channel). The task says "AMD PyTorch-on-Windows, ROCm>=7.2 AMD-hosted wheels". The current file uses a Chinese mirror which is a community fallback — replace with the proper AMD index.
   - `cpu.txt`: Already has `torch>=2.5.0`. Verified correct.

5. **Create `worker/tests/test_defaults.py`**:
   - Import `from worker.defaults import ModelDefaults, ZIT_DEFAULTS, SDXL_DEFAULTS`.
   - Test that `ZIT_DEFAULTS` has `steps=8, guidance_scale=0.0, width=1024, height=1024, dtype="bf16"`.
   - Test that `SDXL_DEFAULTS` has `steps=20, guidance_scale=7.5, width=1024, height=1024, dtype="fp16", supports_negative_prompt=True`.
   - No torch/diffusers import needed — pure dataclass test.

6. **Verify**: Run `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v` to confirm all tests pass (including the new `test_defaults.py`).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `worker/defaults.py` | ModelDefaults dataclass + ZIT_DEFAULTS + SDXL_DEFAULTS |
| Modify | `worker/requirements/base.txt` | Add accelerate, version constraints on diffusers/transformers/Pillow |
| Modify | `worker/requirements/rocm-windows.txt` | Replace Tsinghua mirror with proper AMD PyTorch-on-Windows index |
| Create | `worker/tests/test_defaults.py` | Import test verifying default objects and field values |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `worker/tests/test_defaults.py` | `test_zit_defaults_fields` | ZIT_DEFAULTS has steps=8, guidance_scale=0.0, width=1024, height=1024, dtype="bf16" |
| `worker/tests/test_defaults.py` | `test_sdxl_defaults_fields` | SDXL_DEFAULTS has steps=20, guidance_scale=7.5, width=1024, height=1024, dtype="fp16", supports_negative_prompt=True |
| `worker/tests/test_defaults.py` | `test_model_defaults_default_dtype` | ModelDefaults defaults dtype to "bf16" (if default set) or just validates dataclass structure |

## CI Impact

No CI workflow file changes required. The new test file `worker/tests/test_defaults.py` will be automatically picked up by the existing CI Python worker gate:
```bash
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v
```
This gate already runs for all phases and will include the new test.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `rocm-windows.txt` index URL may change between ROCm releases | Low | Medium | Pin to a specific ROCm version in the URL (e.g. rocm6.2) matching the torch version; note in comments that it should be updated when ROCm/torch versions change |
| `diffusers>=0.27` is very permissive — older versions may lack ZiT support | Medium | Low | The task specifies >=0.27; future tasks (P21-A5) will lock to a specific version if needed. Document in comments. |
| `transformers` 5.x major version shift may break API compatibility with diffusers | Low | Medium | >=4.40 is the task requirement; if 5.x breaks diffusers, pin to `<5.0` in a follow-up task |
| Test imports fail if `worker/__init__.py` is missing or misconfigured | Low | Low | `worker/__init__.py` already exists (empty). Direct `import defaults` from `worker/tests/` works without package init |

## Acceptance Criteria

- [ ] `worker/defaults.py` exists with `ModelDefaults` dataclass, `ZIT_DEFAULTS`, and `SDXL_DEFAULTS` matching design spec §14.6
- [ ] `worker/requirements/base.txt` contains all 8 packages with version constraints as specified (diffusers>=0.27, transformers>=4.40, accelerate, Pillow>=10.0, msgpack>=1.0, numpy, safetensors, pytest)
- [ ] `worker/requirements/cuda.txt` has CUDA index URL + torch>=2.5.0
- [ ] `worker/requirements/rocm-linux.txt` has ROCm index URL + torch>=2.5.0
- [ ] `worker/requirements/rocm-windows.txt` has AMD PyTorch-on-Windows index + torch>=2.5.0
- [ ] `worker/requirements/cpu.txt` has torch>=2.5.0
- [ ] `worker/tests/test_defaults.py` exists and `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v` passes with zero failures
