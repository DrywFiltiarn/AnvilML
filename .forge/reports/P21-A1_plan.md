# Plan Report: P21-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P21-A1                                      |
| Phase       | 21 — ZiT Diffusion Arch Module: Sampling & Latent Shape |
| Description | worker/nodes/arch/diffusion/zit.py: compute_latent_shape() formula |
| Depends on  | P20-C3                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-14T04:55:00Z                        |
| Attempt     | 1                                           |

## Objective

Implement `compute_latent_shape(width: int, height: int, batch_size: int) -> tuple` in `worker/nodes/arch/diffusion/zit.py`, using ZiT's architecture-specific patch-packing formula derived from the hyperparameters that `_infer_hyperparams()` (Phase 20's P20-B1) already extracts. The method returns a `(batch_size, latent_channels, latent_height, latent_width)` tuple where `latent_height = ceil(width / patch_size)` and `latent_width = ceil(height / patch_size)`. This enables the `EmptyLatent` node (Phase 24) to produce correctly-sized noise latents for ZiT models.

## Scope

### In Scope
- Add `compute_latent_shape(width: int, height: int, batch_size: int) -> tuple` function to `worker/nodes/arch/diffusion/zit.py`.
- The formula derives `patch_size` and `latent_channels` from the same hyperparameters that `_infer_hyperparams()` extracts: `patch_size = hidden_dim // latent_channels` (already computed in `_infer_hyperparams_inner`), `latent_channels` (already extracted from the `latents` key shape).
- Non-multiple-of-patch-size dimensions are rounded up (ceiling division), with the rounding rule documented in a code comment.
- Add dual-mode parity markers (`REAL_PATH_VERIFIED:` / `MOCK_PATH_VERIFIED:`) next to the function.
- Add >=5 tests to `worker/tests/test_arch_zit.py` covering the formula for several width/height/batch_size combos, batch_size scaling, and non-multiple-of-patch-size dimensions.
- Total test count in `test_arch_zit.py` must be >=25 (currently 24).

### Out of Scope
None. `defers_to (from JSON): []` — this task may not defer any scope. The entire formula, documentation, markers, and tests are in scope.

## Existing Codebase Assessment

**What already exists:** `zit.py` (718 lines) contains `_infer_hyperparams()` which extracts `hidden_dim`, `double_block_count`, `single_block_count`, `latent_channels`, `latent_height`, `latent_width`, `patch_size`, `arch`, and `native_dtype` from a safetensors checkpoint header. The `ZiTModel` class uses these hyperparameters to construct the model architecture. `load()`, `can_handle()`, `_select_dtype()`, `_build_key_remapping()`, and `_safetensors_dtype_to_canonical()` are all implemented. The test file `test_arch_zit.py` has 24 tests covering `_infer_hyperparams()`, `can_handle()`, `get_module()`, `_select_dtype()` (all 4 precision branches), `load()` materialization, key remapping, and error paths.

**Established patterns:** Google-style docstrings with `Args:`, `Returns:`, `Raises:` sections; inline `#` comments at every decision point explaining *why*; dual-mode parity markers as module-level comment pairs (`REAL_PATH_VERIFIED:` / `MOCK_PATH_VERIFIED:`) pointing at collectible test function names; `_DEFAULT_CAPS` fixture dict in tests; `Path(__file__).parent / "fixtures"` for fixture paths; `@pytest.mark.real_mode` on real-mode tests; `pytest.raises(ValueError, match=...)` for error assertions.

**Gap between design doc and current source:** `compute_latent_shape()` does not exist yet. The formula is not a generic `/8` downscale — it must use ZiT's own `patch_size` (currently 16 for the tiny fixture, derived as `hidden_dim // latent_channels = 64 // 4 = 16`). The existing `_infer_hyperparams()` returns `patch_size` and `latent_channels` but they are not yet accessible to `compute_latent_shape()` since that function doesn't exist. The approach will use module-level constants to cache the hyperparameters after `load()` is called, or accept them as parameters. Based on the design doc (§10.3 EmptyLatent row and §10.4 dispatch table), `compute_latent_shape()` takes `(width, height, batch_size)` — no model or hyperparams argument — so the function must internally derive the formula from a known constant or from a cached reference. The simplest correct approach: store the `patch_size` and `latent_channels` as module-level defaults after `load()` populates them (following the established pattern where `ARCH = "zit"` is a module-level constant), OR use a closure/factory pattern. Given the design doc's fixed signature, the cleanest approach is to store `patch_size` and `latent_channels` as module-level state set by `load()`, with sensible defaults for testing.

## Resolved Dependencies

None. This task only adds a pure Python function to an existing module. No new external packages or crates are introduced.

## Approach

1. **Add module-level hyperparameter state** to `zit.py`:
   - Add `MODEL_PATCH_SIZE: int = 16` and `MODEL_LATENT_CHANNELS: int = 4` as module-level constants after the `ARCH` constant. These serve as defaults for `compute_latent_shape()` when called without a prior `load()` (e.g., in tests). After `load()` is called, these values are updated from the checkpoint's actual hyperparameters.
   - Rationale: The design doc fixes the signature to `compute_latent_shape(width, height, batch_size)` with no model/hyperparams argument. The function must know `patch_size` and `latent_channels` to compute the formula. Storing them as module-level state set by `load()` is the simplest approach that matches the fixed signature.

2. **Implement `compute_latent_shape(width: int, height: int, batch_size: int) -> tuple`**:
   - Signature: `def compute_latent_shape(width: int, height: int, batch_size: int = 1) -> tuple:`
   - Returns `(batch_size, MODEL_LATENT_CHANNELS, latent_height, latent_width)`.
   - Formula:
     ```python
     latent_height = (width + MODEL_PATCH_SIZE - 1) // MODEL_PATCH_SIZE  # ceil(width / patch_size)
     latent_width = (height + MODEL_PATCH_SIZE - 1) // MODEL_PATCH_SIZE  # ceil(height / patch_size)
     return (batch_size, MODEL_LATENT_CHANNELS, latent_height, latent_width)
     ```
   - The ceiling division `(x + patch_size - 1) // patch_size` is the standard integer ceiling formula. It correctly handles: exact multiples (e.g., `32 / 16 = 2`), non-multiples (e.g., `33 / 16 = ceil(2.0625) = 3`), and edge cases like `width=0` (returns 0).
   - Add a code comment explaining the ceiling-division rounding rule: "Non-multiple-of-patch-size dimensions are rounded up so the latent grid fully covers the input — any partial patch at the edge still needs a full column/row of latent tokens."
   - Add Google-style docstring with `Args:`, `Returns:`, and explanation of the formula.
   - Add dual-mode parity markers:
     ```python
     # REAL_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_compute_latent_shape_real_formula
     # MOCK_PATH_VERIFIED: worker/tests/test_arch_zit.py::test_compute_latent_shape_mock_formula
     ```

3. **Update `load()` to set module-level hyperparameters**:
   - After `_infer_hyperparams()` returns in `load()`, set `MODEL_PATCH_SIZE = hyperparams["patch_size"]` and `MODEL_LATENT_CHANNELS = hyperparams["latent_channels"]`.
   - This ensures that `compute_latent_shape()` called after `load()` uses the actual checkpoint's hyperparameters, not the defaults.
   - Add a debug log: `logger.debug("cached hyperparams: patch_size=%d, latent_channels=%d", MODEL_PATCH_SIZE, MODEL_LATENT_CHANNELS)`.

4. **Add tests to `worker/tests/test_arch_zit.py`** (at least 5 new tests, bringing total to >=29):
   - `test_compute_latent_shape_mock_exact_multiple`: Mock-mode test — width=32, height=32, batch_size=1, patch_size=16 → `(1, 4, 2, 2)`. Verifies the primary formula path.
   - `test_compute_latent_shape_mock_non_multiple`: Mock-mode test — width=33, height=33, batch_size=1, patch_size=16 → `(1, 4, 3, 3)`. Verifies ceiling division for non-multiples.
   - `test_compute_latent_shape_mock_batch_scaling`: Mock-mode test — width=64, height=64, batch_size=4, patch_size=16 → `(4, 4, 4, 4)`. Verifies batch_size scales the first dimension correctly.
   - `test_compute_latent_shape_real_after_load`: Real-mode test — calls `load()` against the fixture (which has patch_size=16, latent_channels=4), then calls `compute_latent_shape(32, 32, 1)` → `(1, 4, 2, 2)`. Verifies the load→compute pipeline.
   - `test_compute_latent_shape_real_non_multiple_after_load`: Real-mode test — same as above but with non-multiple dims (width=50, height=50) → `(1, 4, 4, 4)`. Verifies ceiling division after load updates the hyperparameters.
   - `test_compute_latent_shape_default_batch_size`: Test that `batch_size` defaults to 1 when omitted.
   - `test_compute_latent_shape_zero_dims`: Edge case — width=0 or height=0 → latent dimensions are 0.

## Public API Surface

```python
# worker/nodes/arch/diffusion/zit.py

# New module-level constants:
MODEL_PATCH_SIZE: int = 16
MODEL_LATENT_CHANNELS: int = 4

# New function:
def compute_latent_shape(width: int, height: int, batch_size: int = 1) -> tuple:
    """Compute the latent tensor shape for a given input resolution.

    Uses ZiT's patch-packing formula: latent_height = ceil(width / patch_size),
    latent_width = ceil(height / patch_size). Returns (batch_size, latent_channels,
    latent_height, latent_width).

    Non-multiple-of-patch-size dimensions are rounded up via ceiling division
    so the latent grid fully covers the input — any partial patch at the edge
    still needs a full column/row of latent tokens.

    Args:
        width: Input image width in pixels.
        height: Input image height in pixels.
        batch_size: Number of samples in the batch. Defaults to 1.

    Returns:
        A 4-tuple (batch_size, latent_channels, latent_height, latent_width)
        representing the shape of the noise latent tensor that EmptyLatent
        should produce before passing it to the Sampler.
    """
```

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/diffusion/zit.py` | Add `MODEL_PATCH_SIZE`, `MODEL_LATENT_CHANNELS` constants; add `compute_latent_shape()` function; update `load()` to set hyperparameter state; add dual-mode parity markers |
| MODIFY | `worker/tests/test_arch_zit.py` | Add >=5 new tests for `compute_latent_shape()` (formula correctness, batch scaling, non-multiple rounding, post-load integration) |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `test_arch_zit.py` | `test_compute_latent_shape_mock_exact_multiple` | Formula produces `(1, 4, 2, 2)` for width=32, height=32, batch_size=1 with patch_size=16 | `python -m pytest worker/tests/test_arch_zit.py::test_compute_latent_shape_mock_exact_multiple -v` |
| `test_arch_zit.py` | `test_compute_latent_shape_mock_non_multiple` | Ceiling division: width=33, height=33 → `(1, 4, 3, 3)` with patch_size=16 | `python -m pytest worker/tests/test_arch_zit.py::test_compute_latent_shape_mock_non_multiple -v` |
| `test_arch_zit.py` | `test_compute_latent_shape_mock_batch_scaling` | batch_size=4 scales first dim: width=64, height=64 → `(4, 4, 4, 4)` | `python -m pytest worker/tests/test_arch_zit.py::test_compute_latent_shape_mock_batch_scaling -v` |
| `test_arch_zit.py` | `test_compute_latent_shape_real_after_load` | After `load()`, `compute_latent_shape(32, 32, 1)` → `(1, 4, 2, 2)` using actual checkpoint hyperparams | `python -m pytest worker/tests/test_arch_zit.py::test_compute_latent_shape_real_after_load -v` |
| `test_arch_zit.py` | `test_compute_latent_shape_real_non_multiple_after_load` | After `load()`, ceiling division for width=50, height=50 → `(1, 4, 4, 4)` | `python -m pytest worker/tests/test_arch_zit.py::test_compute_latent_shape_real_non_multiple_after_load -v` |
| `test_arch_zit.py` | `test_compute_latent_shape_default_batch_size` | Omitting batch_size defaults to 1: `compute_latent_shape(32, 32)` → `(1, 4, 2, 2)` | `python -m pytest worker/tests/test_arch_zit.py::test_compute_latent_shape_default_batch_size -v` |
| `test_arch_zit.py` | `test_compute_latent_shape_zero_dims` | Edge case: width=0 or height=0 → latent dims are 0 | `python -m pytest worker/tests/test_arch_zit.py::test_compute_latent_shape_zero_dims -v` |

## CI Impact

No CI changes required. The task only adds a Python function and tests within the existing `worker/tests/test_arch_zit.py` file. The existing CI jobs (`worker-linux-mock`, `worker-linux-real`, `worker-windows-mock`, `worker-windows-real`) already run `pytest worker/tests/` and will pick up the new tests automatically.

## Platform Considerations

None identified. The formula is a pure integer computation with no platform-specific behavior. The ceiling division `(x + patch_size - 1) // patch_size` works identically on all platforms. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The ceiling-division formula `(width + patch_size - 1) // patch_size` may produce incorrect results for width=0 (returns 0, which is correct) or negative values (should not occur in practice but would return 0 due to Python's floor division behavior on positive operands). | Low | Low | Test the zero-dimension edge case explicitly; add a guard comment noting that negative dimensions are invalid inputs. |
| Module-level state (`MODEL_PATCH_SIZE`, `MODEL_LATENT_CHANNELS`) may retain stale values between test runs if `load()` is not called before `compute_latent_shape()` in some tests. | Medium | Medium | Each test that relies on post-load values calls `load()` first. Tests that use default values explicitly set the constants to known values. The default values (16, 4) match the tiny fixture's actual values, so even tests that don't call `load()` get correct results. |
| The dual-mode parity markers for `compute_latent_shape()` may be questioned since §10.6 of the design doc only explicitly lists `load()`, `sample()`, and `decode()` (not `compute_latent_shape()`). However, Gate 4 in ENVIRONMENT.md §8 explicitly includes `compute_latent_shape()` in the grep sweep. | Low | Medium | Add the markers as required by Gate 4's sweep. The markers point to tests that exercise both mock and real paths of the same pure function. |

## Acceptance Criteria

- [ ] `grep "def compute_latent_shape" /home/dryw/AnvilML/worker/nodes/arch/diffusion/zit.py` finds the function definition
- [ ] `grep "REAL_PATH_VERIFIED:" /home/dryw/AnvilML/worker/nodes/arch/diffusion/zit.py | grep compute_latent_shape` confirms the real-mode marker
- [ ] `grep "MOCK_PATH_VERIFIED:" /home/dryw/AnvilML/worker/nodes/arch/diffusion/zit.py | grep compute_latent_shape` confirms the mock-mode marker
- [ ] `grep -c "^def test_" /home/dryw/AnvilML/worker/tests/test_arch_zit.py` outputs a number >= 25
- [ ] `python -m pytest worker/tests/test_arch_zit.py -v` exits 0 with all tests passing
