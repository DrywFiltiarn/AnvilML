# Plan Report: P18-D3b

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-D3b                                     |
| Phase       | 018 — ZiT Generic Nodes                     |
| Description | worker/nodes/arch/zit.py: add compute_latent_shape() function |
| Depends on  | P18-D3 (VAE_SCALE_FACTOR constant), P18-D2 (get_module dispatcher) |
| Project     | anvilml                                     |
| Planned at  | 2026-06-22T09:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Add the `compute_latent_shape()` function to `worker/nodes/arch/zit.py` that implements the exact latent shape formula used by `ZImagePipeline.prepare_latents`. This function computes `(batch_size, num_channels_latents, h, w)` where `h = 2 * (height // (VAE_SCALE_FACTOR * 2))` and `w = 2 * (width // (VAE_SCALE_FACTOR * 2))`. It becomes `EmptyLatent`'s (P18-D8) actual dispatch target via the architecture module, replacing a bare `VAE_SCALE_FACTOR` lookup with an architecture-specific shape formula. Two unit tests verify correctness on known dimensions and a non-divisible edge case.

## Scope

### In Scope
- Add `def compute_latent_shape(batch_size: int, height: int, width: int, num_channels_latents: int) -> tuple[int, ...]` to `worker/nodes/arch/zit.py`.
- Append `"compute_latent_shape"` to `__all__` in `zit.py`.
- Add a Google-style docstring with `Args:` and `Returns:` sections.
- Add ≥ 2 unit tests in `worker/tests/test_arch_zit.py`:
  - Correct shape for a known height/width pair (1024×1024 → latent shape `(1, 4, 128, 128)`).
  - Non-divisible-by-scale-factor edge case (height or width not cleanly divisible by `VAE_SCALE_FACTOR * 2`).
- Python syntax check (`py_compile`) passes for `worker/nodes/arch/zit.py`.

### Out of Scope
- Wiring `compute_latent_shape()` into `EmptyLatent`'s real path — that is the responsibility of P18-D8.
- Any changes to `worker/nodes/sampler.py` or `worker/nodes/base.py`.
- Changes to `worker/nodes/arch/__init__.py`.
- Rust crate changes, CI configuration, or config file updates.
- Real-mode sampling path implementation (P18-D9a–c).

## Existing Codebase Assessment

The `worker/nodes/arch/zit.py` module already exists with `VAE_SCALE_FACTOR = 8`, `MockLatent`, `can_handle()`, and a mock `sample()` stub. The module follows established patterns: Google-style docstrings on all public items, inline `#` comments explaining decision points, lazy imports guarded by `os.environ.get("ANVILML_WORKER_MOCK")`, and `__all__` listing public symbols.

The test file `worker/tests/test_arch_zit.py` already has 7 tests covering `VAE_SCALE_FACTOR`, `can_handle()` (ZiT and non-ZiT), mock `sample()` (seed preservation, real-mode NotImplementedError), and import isolation. Tests use a `_make_model()` helper for mock objects, follow Google-style docstrings with Preconditions/Tests/Expected output sections, and respect the `conftest.py` autouse `mock_mode` fixture.

The design doc (`ANVILML_DESIGN.md §10.4`) specifies the exact function signature and contract: architecture modules expose `compute_latent_shape(batch_size, height, width, num_channels_latents) -> tuple[int, ...]`, and generic nodes call this on the dispatched module. The shape formula for ZiT is `h = 2 * (height // (VAE_SCALE_FACTOR * 2))`, `w = 2 * (width // (VAE_SCALE_FACTOR * 2))`, derived from `ZImagePipeline.prepare_latents`.

No gap exists between design and source — the module structure, naming conventions, and test patterns are all consistent with this task's requirements.

## Resolved Dependencies

None. This task introduces no new external dependencies. It is a pure Python function using only built-in types (`int`, `tuple`) and the existing `VAE_SCALE_FACTOR` constant.

## Approach

1. **Implement `compute_latent_shape()` in `worker/nodes/arch/zit.py`.**
   - Add the function after the `VAE_SCALE_FACTOR` constant definition (line 33) and before the `MockLatent` class (line 36), to keep constants and pure utility functions grouped together.
   - The formula: `h = 2 * (height // (VAE_SCALE_FACTOR * 2))`, `w = 2 * (width // (VAE_SCALE_FACTOR * 2))`, return `(batch_size, num_channels_latents, h, w)`.
   - Include a Google-style docstring: one-sentence summary, `Args:` section listing all four parameters with descriptions, `Returns:` section describing the tuple layout and its correspondence to `ZImagePipeline.prepare_latents` validation.
   - Add a `# defers_to: P18-D8 — consumed by EmptyLatent real path` inline comment at the function body, since P18-D8 is the named recipient that will wire this into `EmptyLatent`.

2. **Update `__all__` in `zit.py`.**
   - Append `"compute_latent_shape"` to the existing `__all__` list on line 27, maintaining alphabetical order: `["can_handle", "compute_latent_shape", "sample", "MockLatent", "VAE_SCALE_FACTOR"]`.

3. **Add two unit tests in `worker/tests/test_arch_zit.py`.**
   - **Test 1 — `test_compute_latent_shape_known_dims`:** Call `compute_latent_shape(1, 1024, 1024, 4)` and assert the result equals `(1, 4, 128, 128)`. This is the canonical ZiT case: 1024×1024 image → 128×128 latent (8× spatial compression), batch 1, 4 channels (standard SD-style).
   - **Test 2 — `test_compute_latent_shape_non_divisible`:** Call `compute_latent_shape(2, 1025, 1026, 4)` and assert the result equals `(2, 4, 128, 128)`. The floor division `1025 // 16 = 64` and `1026 // 16 = 64`, so `h = w = 128` — this verifies that non-divisible dimensions silently floor rather than raise, matching `ZImagePipeline.prepare_latents`'s integer-division behavior.

4. **Run Python syntax check.**
   - Execute `worker/.venv/bin/python -m py_compile worker/nodes/arch/zit.py` to confirm no syntax errors before running pytest.

5. **Run the test suite.**
   - Execute `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v` and confirm all tests pass (existing 7 + 2 new = 9).

## Public API Surface

| Item | Module Path | Signature |
|------|-------------|-----------|
| `compute_latent_shape` | `worker.nodes.arch.zit` | `def compute_latent_shape(batch_size: int, height: int, width: int, num_channels_latents: int) -> tuple[int, ...]` |

This is a new `pub` (Python-level public, exported via `__all__`) function. No existing public items are modified.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/zit.py` | Add `compute_latent_shape()` function; update `__all__` |
| MODIFY | `worker/tests/test_arch_zit.py` | Add 2 new unit tests |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_arch_zit.py` | `test_compute_latent_shape_known_dims` | Correct shape computation for the canonical ZiT case (1024×1024 → 128×128 latent) | `ANVILML_WORKER_MOCK=1` (from conftest.py autouse fixture) | `batch_size=1, height=1024, width=1024, num_channels_latents=4` | `(1, 4, 128, 128)` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_compute_latent_shape_known_dims -v` exits 0 |
| `worker/tests/test_arch_zit.py` | `test_compute_latent_shape_non_divisible` | Floor division handles non-divisible dimensions without raising | `ANVILML_WORKER_MOCK=1` (from conftest.py autouse fixture) | `batch_size=2, height=1025, width=1026, num_channels_latents=4` | `(2, 4, 128, 128)` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py::test_compute_latent_shape_non_divisible -v` exits 0 |

## CI Impact

No CI changes required. The new tests are picked up by the existing `worker-linux` and `worker-windows` CI jobs, which already run `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v`. No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. The function is a pure arithmetic computation using Python built-in integer floor division (`//`). It has no I/O, no platform-specific paths, and no file system or network operations. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The formula `2 * (height // (VAE_SCALE_FACTOR * 2))` may produce `h=0` or `w=0` for very small input dimensions (e.g. height < 16), which would create an invalid latent shape that `ZImagePipeline.prepare_latents` rejects with a `ValueError`. | Low | Medium | The function is a pure computation with no validation — this matches the design contract which places shape validation responsibility on the caller (`EmptyLatent` via `ZImagePipeline.prepare_latents`). Document this in the docstring so the consumer knows to validate inputs before calling. |
| The test for non-divisible dimensions may produce an unexpected result if the actual `ZImagePipeline.prepare_latents` formula differs from what is documented (e.g. if it uses `ceil` instead of `floor` division). | Low | Medium | The task context explicitly states this is per `ZImagePipeline.prepare_latents`'s formula, and the design doc confirms integer floor division. If the ACT agent discovers a discrepancy at implementation time, it must confirm against the actual `diffusers` source and adjust the formula — the plan's formula is verified via the task context's specification, not from memory. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/arch/zit.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_arch_zit.py -v` exits 0 with 9 tests (7 existing + 2 new)
- [ ] `grep '"compute_latent_shape"' worker/nodes/arch/zit.py` returns the updated `__all__` line
- [ ] `grep 'def compute_latent_shape' worker/nodes/arch/zit.py` returns the function definition with the correct signature
