# Plan Report: P18-D3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-D3                                        |
| Phase       | 018 — ZiT Generic Nodes                       |
| Description | worker/nodes/arch/zit.py: add VAE_SCALE_FACTOR module constant |
| Depends on  | P18-D2                                        |
| Project     | anvilml                                       |
| Planned at  | 2026-06-22T07:25:00Z                          |
| Attempt     | 1                                             |

## Objective

Add a module-level constant `VAE_SCALE_FACTOR: int = 8` to `worker/nodes/arch/zit.py`, with an inline comment citing its origin from Z-Image-Turbo's published VAE config. Export it via `__all__`. Add one unit test in `worker/tests/test_arch_zit.py` asserting the value equals `8`. This constant is consumed later by `EmptyLatent`'s real path (P18-D8) via the P18-D2 dispatcher (`arch.get_module(model).VAE_SCALE_FACTOR`), but wiring that consumer is out of scope.

## Scope

### In Scope
- Add `VAE_SCALE_FACTOR: int = 8` as a module-level constant in `worker/nodes/arch/zit.py`, immediately after the `__all__` declaration, with an inline comment citing the source (Z-Image-Turbo VAE config `block_out_channels=[128,256,512,512]`, 4 entries, `2**(4-1)=8`).
- Add `"VAE_SCALE_FACTOR"` to `__all__` in `worker/nodes/arch/zit.py`.
- Add one test `test_vae_scale_factor_value` in `worker/tests/test_arch_zit.py` asserting `VAE_SCALE_FACTOR == 8`.

### Out of Scope
- Wiring `VAE_SCALE_FACTOR` into `EmptyLatent`'s real path (P18-D8).
- Adding `compute_latent_shape()` (P18-D3b, a separate downstream task).
- Any changes to `arch/__init__.py` or other arch modules.
- Any changes to Rust crates or CI configuration.

## Existing Codebase Assessment

The `worker/nodes/arch/zit.py` module (148 lines) already provides `can_handle()`, `sample()`, and `MockLatent` with a comprehensive module-level docstring, Google-style docstrings on all public items, and inline comments at decision points. The `__all__` list currently exports `["can_handle", "sample", "MockLatent"]`. The module follows a strict import-isolation pattern: `torch`, `diffusers`, and `safetensors` are never imported at module level; real-mode imports are lazy inside the `if not _mock:` guard within `sample()`.

The test file `worker/tests/test_arch_zit.py` (265 lines) contains 5 tests covering `can_handle()` dispatch, mock `sample()` return values, real-mode `NotImplementedError`, and import isolation. All tests use the `conftest.py` autouse `mock_mode` fixture which sets `ANVILML_WORKER_MOCK=1` and restores the original value unconditionally. Test style uses simple `assert` statements with docstrings describing preconditions, test steps, and expected output.

No gap exists between the design doc and current source: the module structure is clean and adding a module-level constant follows the established pattern. No external dependencies are introduced.

## Resolved Dependencies

None. This task adds a pure Python module-level constant with no new imports or external package references.

## Approach

1. **Add `VAE_SCALE_FACTOR` to `__all__`** in `worker/nodes/arch/zit.py`: modify the existing `__all__ = ["can_handle", "sample", "MockLatent"]` line to `__all__ = ["can_handle", "sample", "MockLatent", "VAE_SCALE_FACTOR"]`. This ensures the constant is part of the module's public API surface and is discoverable by the P18-D2 dispatcher when consumers access `mod.VAE_SCALE_FACTOR` via `arch.get_module(model)`.

2. **Add the constant with inline comment** immediately after the `__all__` declaration, following the established module structure (constant declarations sit between `__all__` and the first class/function). The exact code to insert:
   ```python
   # Z-Image-Turbo's published VAE config has block_out_channels=[128,256,512,512]
   # (4 entries), giving 2**(4-1)=8 per ZImagePipeline.__init__'s vae_scale_factor
   # formula; independently corroborated as 8x spatial compression
   # (1024x1024 image -> 128x128 latent grid).
   VAE_SCALE_FACTOR: int = 8
   ```
   This satisfies FORGE_AGENT_RULES §12.2 (magic number/constant must have an inline comment explaining origin or meaning) and ENVIRONMENT.md §10 (non-trivial decision point comment).

3. **Add test** in `worker/tests/test_arch_zit.py`: add a new test function `test_vae_scale_factor_value` after the existing helper section and before the `can_handle` tests. The test imports `VAE_SCALE_FACTOR` from the module under test, asserts it equals `8`, and includes a docstring following the established test style (preconditions, test steps, expected output). No environment variable mutation is needed since reading a module-level constant does not affect process-global state. The test file does not need `#[serial]` or serial pytest grouping because no env vars are mutated.

4. **Run syntax check** (`worker/.venv/bin/python -m py_compile worker/nodes/arch/zit.py worker/tests/test_arch_zit.py`) to confirm no syntax errors before running tests, per ENVIRONMENT.md §7 (mandatory pre-test check for Python tasks).

5. **Run the test suite** (`ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v`) to confirm all existing tests still pass and the new test passes.

## Public API Surface

| Item | Type | Module Path | Description |
|------|------|-------------|-------------|
| `VAE_SCALE_FACTOR` | `int` (module-level constant) | `worker.nodes.arch.zit.VAE_SCALE_FACTOR` | VAE spatial compression factor for Z-Image Turbo; value is `8`. |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/arch/zit.py` | Add `VAE_SCALE_FACTOR` constant and append to `__all__` |
| Modify | `worker/tests/test_arch_zit.py` | Add `test_vae_scale_factor_value` test |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_arch_zit.py` | `test_vae_scale_factor_value` | `VAE_SCALE_FACTOR` module constant equals `8` | `ANVILML_WORKER_MOCK=1` (set by conftest.py autouse fixture) | None (reads module-level constant) | `VAE_SCALE_FACTOR == 8` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v` exits 0 |

## CI Impact

No CI changes required. The task modifies existing Python source and test files that are already picked up by the `worker-linux` and `worker-windows` CI jobs (py_compile + pytest). No new file types, gates, or test modules are introduced. The existing `worker/tests/test_arch_zit.py` test file is already part of the full test suite run by `pytest worker/tests/`.

## Platform Considerations

None identified. The constant is a pure Python integer literal with no platform-specific behavior, path handling, or conditional compilation. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The inline comment text is long enough that a line-wrapping formatter (black/isort) might reflow it, potentially changing the exact wording from what the task context specifies. | Low | Low | The comment uses `#` prefix (Python comment), so the Python formatter does not touch it. No risk of reflow. |
| Adding `VAE_SCALE_FACTOR` to `__all__` could cause an `ImportError` in existing code that does `from worker.nodes.arch.zit import *` if the constant is not defined before the import is evaluated. | Very Low | Low | The constant is defined in the same edit pass, immediately after `__all__`. Python evaluates `__all__` at import time but the name must already be bound — since we define it before any code that might trigger `import *` evaluation, this is safe. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/arch/zit.py worker/tests/test_arch_zit.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v` exits 0
- [ ] `grep 'VAE_SCALE_FACTOR' worker/nodes/arch/zit.py | grep -q 'int = 8'` — constant is defined with correct type annotation and value
- [ ] `grep '"VAE_SCALE_FACTOR"' worker/nodes/arch/zit.py | grep -q '__all__'` — constant is exported in `__all__`
- [ ] `grep 'test_vae_scale_factor_value' worker/tests/test_arch_zit.py` — new test function exists
