# Plan Report: P23-C2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P23-C2                                      |
| Phase       | 23 — ZiT VAE Arch Module                    |
| Description | worker/nodes/arch/vae/zit_vae.py: dtype selection per InferenceCaps |
| Depends on  | P23-C1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-17T10:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Add four new tests to `worker/tests/test_arch_vae_zit.py` that exercise each of the four
dtype-precedence branches (fp8, bf16, fp16, fp32) in the existing `_select_dtype()`
function through `load()`, producing a `ZiTVaeModel` at the expected dtype. This task
does not modify source code — the `_select_dtype()` function and its integration into
`load()` were already implemented in P23-C1. The sole deliverable is test coverage
proving the precedence chain works correctly for every branch.

## Scope

### In Scope
- Four new tests in `worker/tests/test_arch_vae_zit.py`, each exercising one precedence
  branch of `_select_dtype()` via `load()`:
  - fp8: `caps.fp8=True` AND checkpoint native dtype is `fp8` → `torch.float8_e4m3fn`
  - bf16: `caps.bf16=True` (native dtype irrelevant) → `torch.bfloat16`
  - fp16: only `caps.fp16=True` → `torch.float16`
  - fp32: all capability flags `False` → `torch.float32`
- A new FP8 VAE fixture builder (`worker/tests/fixtures/build_zit_vae_fp8_fixture.py`)
  and its output file (`worker/tests/fixtures/zit_vae_tiny_fp8.safetensors`) to exercise
  the fp8 branch (the existing fixtures are float32, so fp8 native dtype requires a
  dedicated fixture).
- Update `docs/TESTS.md` with entries for the four new tests.

### Out of Scope
- Source code changes to `zit_vae.py` — `_select_dtype()` and its integration in `load()`
  are already implemented by P23-C1.
- Dual-mode parity markers on `load()` — these are deferred to P23-C3 when `load()`
  is completed (materialization, key remap, `load_state_dict`). The `load()` function
  currently returns a meta-device-only module; the parity markers require a fully
  functional load path.
- fp8 fixture creation for the diffusion `zit.py` module — that already exists
  (`zit_tiny_fp8.safetensors`).

## Existing Codebase Assessment

**What already exists:** P23-C1 delivered `_select_dtype()` (lines 445–485 of
`zit_vae.py`) implementing the fp8→bf16→fp16→fp32 precedence chain, and `load()`
calls `_select_dtype()` at line 554 and applies the result via `model.to(target_dtype)`
at line 567. The function reads `caps` as a plain `dict` (matching how `NodeContext.caps`
is passed from `probe_capabilities()`), and `native_dtype` as a canonical lowercase
string from `_infer_hyperparams()`.

The existing test file has 11 tests: 4 for `_infer_hyperparams()`, 3 for dispatch
(`can_handle`, `get_module`, `ARCH`), and 3 for `load()` — `test_load_meta_construction_succeeds`
(which exercises the bf16 branch), `test_load_meta_construction_no_metadata_fixture`
(also bf16 via the same caps), and `test_load_dtype_selection_applied` (fp32 fallback).

**Established patterns:** Tests use guarded `import torch` (try/except with sentinel
`None`) to remain importable in mock-mode collection. Real-mode tests are marked
`@pytest.mark.real_mode`. Fixture paths use `Path(__file__).parent / "fixtures"`.
Dtype assertions check `param.dtype` on meta-device parameters (dtype metadata, not
actual tensor data).

**Gap:** No FP8 fixture exists for the VAE module. The existing fixtures are built with
`torch.randn()` which defaults to `float32`. To exercise the fp8 branch of
`_select_dtype()` (which requires `native_dtype == "fp8"`), a fixture with FP8 tensors
is needed. The diffusion module's `build_zit_fp8_fixture.py` serves as the pattern.

## Resolved Dependencies

None. This task adds no new dependencies. It exercises existing `torch` types
(`torch.float8_e4m3fn`, `torch.bfloat16`, `torch.float16`, `torch.float32`) that are
already imported by the module's guarded import block.

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| python | torch   | (project venv)  | n/a            | n/a                    |

No external crates or packages are introduced or referenced. All types used are from
the Python standard library, `torch`, `safetensors`, and `pytest` — all already in the
project's requirements.

## Approach

1. **Create the FP8 VAE fixture builder.** Copy the pattern from
   `worker/tests/fixtures/build_zit_fp8_fixture.py` (which creates `zit_tiny_fp8.safetensors`
   for the diffusion module) to create
   `worker/tests/fixtures/build_zit_vae_fp8_fixture.py`. The builder creates a
   `zit_vae_tiny_fp8.safetensors` file with the same tensor shapes as the regular VAE
   fixture but with all tensors converted to `torch.float8_e4m3fn` via `.to()` on
   float32 intermediates (torch.randn does not support float8 directly on CPU builds).
   The metadata includes `{"arch": "zit_vae"}`. Run the builder to produce the fixture
   file.

2. **Add test for fp8 branch.** Write `test_load_dtype_fp8_caps_and_native`:
   - Import `load` and `ZiTVaeModel` from `zit_vae`.
   - Use the new `zit_vae_tiny_fp8.safetensors` fixture.
   - Pass `caps = {"fp8": True, "bf16": False, "fp16": False, "fp32": True}`.
   - Assert the model is a `ZiTVaeModel` with all parameters at `torch.float8_e4m3fn`.
   - Mark `@pytest.mark.real_mode` (requires torch).

3. **Add test for bf16 branch.** Write `test_load_dtype_bf16_caps_selects_bf16`:
   - Use the existing `zit_vae_tiny.safetensors` fixture (native_dtype=fp32).
   - Pass `caps = {"bf16": True, "fp16": True, "fp8": False, "fp32": True}`.
   - Assert parameters at `torch.bfloat16`.
   - Mark `@pytest.mark.real_mode`.
   - (This is a distinct test from `test_load_meta_construction_succeeds` which also
     uses bf16 but is primarily a meta-construction test — this one focuses on dtype
     selection as the primary assertion.)

4. **Add test for fp16 branch.** Write `test_load_dtype_fp16_caps_selects_fp16`:
   - Use the existing `zit_vae_tiny.safetensors` fixture.
   - Pass `caps = {"bf16": False, "fp16": True, "fp8": False, "fp32": True}`.
   - Assert parameters at `torch.float16`.
   - Mark `@pytest.mark.real_mode`.

5. **Add test for fp32 fallback.** Write `test_load_dtype_fp32_fallback`:
   - Use the existing `zit_vae_tiny.safetensors` fixture.
   - Pass `caps = {"bf16": False, "fp16": False, "fp8": False, "fp32": True}`.
   - Assert parameters at `torch.float32`.
   - Mark `@pytest.mark.real_mode`.
   - (The existing `test_load_dtype_selection_applied` already covers this branch, but
     the acceptance criterion says "each of the 4 precedence branches is exercised" —
     having a dedicated test per branch makes the coverage unambiguous.)

6. **Update `docs/TESTS.md`** with entries for the four new tests, including `Mode: real`
   for each (they all require torch, cannot run in mock-mode collection).

7. **Run acceptance command:** `python -m pytest worker/tests/test_arch_vae_zit.py -v`
   must exit 0 with >=14 total tests (11 existing + 4 new = 15).

## Public API Surface

No new public API items. The `_select_dtype()` function already exists and is not
modified. All changes are in test files and a fixture builder.

| Module | Item | Kind |
|--------|------|------|
| `worker/tests/fixtures/build_zit_vae_fp8_fixture.py` | `build()` | New function (script entry point) |
| `worker/tests/fixtures/zit_vae_tiny_fp8.safetensors` | (file) | New fixture |
| `worker/tests/test_arch_vae_zit.py` | `test_load_dtype_fp8_caps_and_native` | New test |
| `worker/tests/test_arch_vae_zit.py` | `test_load_dtype_bf16_caps_selects_bf16` | New test |
| `worker/tests/test_arch_vae_zit.py` | `test_load_dtype_fp16_caps_selects_fp16` | New test |
| `worker/tests/test_arch_vae_zit.py` | `test_load_dtype_fp32_fallback` | New test |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/tests/fixtures/build_zit_vae_fp8_fixture.py` | FP8 VAE fixture builder script |
| CREATE | `worker/tests/fixtures/zit_vae_tiny_fp8.safetensors` | FP8 VAE checkpoint fixture |
| MODIFY | `worker/tests/test_arch_vae_zit.py` | Add 4 dtype-branch tests |
| MODIFY | `docs/TESTS.md` | Add entries for 4 new tests |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `worker/tests/test_arch_vae_zit.py` | `test_load_dtype_fp8_caps_and_native` | caps.fp8=True AND native_dtype=fp8 → `torch.float8_e4m3fn` (fp8 branch) | `python -m pytest worker/tests/test_arch_vae_zit.py::test_load_dtype_fp8_caps_and_native -v` |
| `worker/tests/test_arch_vae_zit.py` | `test_load_dtype_bf16_caps_selects_bf16` | caps.bf16=True → `torch.bfloat16` (bf16 branch) | `python -m pytest worker/tests/test_arch_vae_zit.py::test_load_dtype_bf16_caps_selects_bf16 -v` |
| `worker/tests/test_arch_vae_zit.py` | `test_load_dtype_fp16_caps_selects_fp16` | caps.fp16=True (bf16=False) → `torch.float16` (fp16 branch) | `python -m pytest worker/tests/test_arch_vae_zit.py::test_load_dtype_fp16_caps_selects_fp16 -v` |
| `worker/tests/test_arch_vae_zit.py` | `test_load_dtype_fp32_fallback` | all caps False → `torch.float32` (fp32 fallback) | `python -m pytest worker/tests/test_arch_vae_zit.py::test_load_dtype_fp32_fallback -v` |
| (all) | Full suite | >=14 total tests, all pass | `python -m pytest worker/tests/test_arch_vae_zit.py -v` |

## CI Impact

No CI changes required. The new tests are marked `@pytest.mark.real_mode` and use
guarded torch imports, so they are collected in mock-mode CI (worker-linux-mock,
worker-windows-mock) but only run in real-mode CI (worker-linux-real,
worker-windows-real). No new CI jobs or gates are needed.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The
`torch.float8_e4m3fn` type exists on all torch builds (CPU, CUDA, ROCm). On CPU
builds, fp8 compute is emulated but the dtype object itself is available.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `torch.float8_e4m3fn` not available on the specific torch build installed in the venv | Low | High | The fixture builder uses `.to(torch.float8_e4m3fn)` on float32 intermediates — if this fails, the builder itself will error during fixture creation, making it immediately visible. The test file guards the torch import. |
| FP8 tensors on CPU torch build produce unexpected dtype metadata on meta-device parameters | Low | Medium | On meta device, `model.to(torch.float8_e4m3fn)` sets dtype metadata without executing compute. The assertion checks `param.dtype == torch.float8_e4m3fn` which compares dtype objects, not compute results. |
| Fixture file size exceeds 10 MB budget | Very Low | Low | The builder uses the same small shapes as the regular fixture (16–32 channels, 3×3 convs). FP8 tensors are half the size of float32. Total should be well under 1 MB. |

## Acceptance Criteria

- [ ] `python worker/tests/fixtures/build_zit_vae_fp8_fixture.py` exits 0 and produces
      `worker/tests/fixtures/zit_vae_tiny_fp8.safetensors`
- [ ] `python -m pytest worker/tests/test_arch_vae_zit.py::test_load_dtype_fp8_caps_and_native -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_vae_zit.py::test_load_dtype_bf16_caps_selects_bf16 -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_vae_zit.py::test_load_dtype_fp16_caps_selects_fp16 -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_vae_zit.py::test_load_dtype_fp32_fallback -v` exits 0
- [ ] `python -m pytest worker/tests/test_arch_vae_zit.py -v` exits 0 with >=14 tests collected
