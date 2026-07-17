# Plan Report: P24-B2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P24-B2                                      |
| Phase       | 24 — Generic Conditioning/Sampling/Decode Nodes, Real Mode |
| Description | worker/nodes/decode.py: VaeDecode real branch dispatches to vae module |
| Depends on  | P24-B1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-17T20:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Complete `VaeDecode.execute()`'s real branch in `worker/nodes/decode.py` by replacing the `NotImplementedError` stub with actual dispatch to the loaded VAE architecture module via `arch.vae.get_module(vae.arch).decode(vae, latent)`. Both `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` markers on the `execute()` method must name genuinely passing tests. The acceptance criterion is `>=5 new tests` in `test_nodes_decode.py` with `>=8 total tests` passing under `python -m pytest worker/tests/test_nodes_decode.py -v -m real_mode`.

## Scope

### In Scope
- Replace the `NotImplementedError` stub in `VaeDecode.execute()` real branch with dispatch to `arch.vae.get_module(inputs["vae"].arch).decode(inputs["vae"], inputs["latent"])`.
- Update the `REAL_PATH_VERIFIED` marker comment on `execute()` to name the new real-mode test function.
- Add >=5 new tests to `worker/tests/test_nodes_decode.py`:
  - Real-mode test: load VAE fixture via `load()`, create latent tensor, execute `VaeDecode`, assert real PIL Image output.
  - Real-mode test: batched latent (batch > 1) produces multiple PIL Images.
  - Real-mode test: output image dimensions match latent spatial dimensions.
  - Real-mode test: output PIL Image mode is "RGB".
  - Real-mode test: `vae.arch` attribute is correctly read and used for dispatch.
  - Real-mode test: error when `vae` input lacks an `.arch` attribute.
  - Real-mode test: real-mode `execute()` raises `RuntimeError` when `arch.vae.get_module()` returns `None` (unregistered arch key).
- Update the `MOCK_PATH_VERIFIED` marker is already correct (points at `test_vae_decode_mock_returns_sentinel` which exists and passes).

### Out of Scope
- Modifying `arch/vae/zit_vae.py`'s `decode()` function — it is fully implemented in Phase 23 (P23-D1).
- Modifying `arch/vae/__init__.py`'s `get_module()` dispatcher — it is fully implemented in Phase 23.
- Adding tests for the VAE load path — those are covered by `test_arch_vae_zit.py`.
- Modifying any Rust crates, CI files, or configuration.
- Adding new fixture files — the Phase 23 `zit_vae_tiny.safetensors` fixture is already available.

## Existing Codebase Assessment

**What exists:** `worker/nodes/decode.py` contains the `VaeDecode` class with mock branch fully implemented (returns sentinel dict with latent shape) and a real branch stub that raises `NotImplementedError` with a `defers_to: P24-B2` comment. The class attributes, input/output slots, and registration via `@register` are all correct. The existing markers on `execute()` name `test_vae_decode_real_decodes_zit_vae_fixture` (REAL, not yet written) and `test_vae_decode_mock_returns_sentinel` (MOCK, exists in test file).

**Established patterns:**
- Node execute() branches on `ctx.mock` once at the top — mock returns sentinel, real dispatches to arch module.
- Real-mode tests use `@pytest.mark.real_mode` and load fixtures from `worker/tests/fixtures/`.
- Tests use `unittest.mock.patch` for mock-mode variants that need to bypass real torch calls.
- The `arch.vae.get_module(key)` dispatcher returns `ModuleType | None` — callers must handle `None`.
- `arch.vae.zit_vae.decode(vae_module, latent, output_mode="RGB")` returns `list[PIL.Image.Image]`.
- The loaded VAE module has `.arch == "zit_vae"` set by the `load()` function.
- Error handling: `arch.vae.get_module()` returns `None` for unregistered keys; `decode()` raises `RuntimeError` if torch is absent.

**Gap between design doc and source:** The design doc (ANVILML_DESIGN.md §10.4) states that generic nodes dispatch via `arch.{family}.get_module()` — the current stub in decode.py is a placeholder awaiting this implementation. No discrepancy exists beyond the deferred stub.

## Resolved Dependencies

| Type   | Name     | Version verified | MCP source | Feature flags confirmed |
|--------|----------|-----------------|------------|------------------------|
| python | torch    | (project venv)  | n/a        | n/a                    |
| python | Pillow   | (project venv)  | n/a        | n/a                    |
| python | numpy    | (project venv)  | n/a        | n/a                    |

No new external dependencies are introduced. This task only uses existing imports already present in `decode.py` (via `arch.vae.get_module` and `arch.vae.zit_vae.decode` which are already imported in the dispatcher and arch module).

## Approach

### Step 1: Implement the real branch in `worker/nodes/decode.py`

Replace the `NotImplementedError` stub with the actual dispatch logic:

```python
# In VaeDecode.execute(), replace the else branch:
else:
    vae = inputs.get("vae")
    latent = inputs.get("latent")

    if vae is None:
        raise ValueError("VaeDecode: 'vae' input is required (missing or None)")
    if latent is None:
        raise ValueError("VaeDecode: 'latent' input is required (missing or None)")

    # Get the architecture key from the loaded VAE module.
    # The vae object is the fully-loaded module from LoadVae's real branch;
    # it carries an .arch attribute set by the arch module's load() function.
    arch_key = getattr(vae, "arch", None)
    if arch_key is None:
        raise ValueError(
            f"VaeDecode: vae input has no .arch attribute "
            f"(type={type(vae).__name__}); expected a loaded arch module"
        )

    # Dispatch to the registered VAE architecture module.
    # get_module returns None for unregistered keys — this is the correct
    # failure mode: if a new arch module is registered without a corresponding
    # node update, the error is explicit rather than a silent crash.
    vae_module = arch.vae.get_module(arch_key)
    if vae_module is None:
        raise RuntimeError(
            f"VaeDecode: no registered VAE module handles arch={arch_key!r}; "
            f"check that the arch module is importable and can_handle() returns True"
        )

    # Call the architecture-specific decode function.
    # decode(vae_module, latent) returns list[PIL.Image.Image].
    images = vae_module.decode(vae, latent)

    logger.debug("VaeDecode: real mode, decoded %d image(s)", len(images))
    return {"image": images}
```

Key decisions:
- **Read `vae.arch` via `getattr(vae, "arch", None)`**: The `vae` input is the fully-loaded module from `LoadVae`. It has `.arch` set by `load()` (Phase 23). Using `getattr` with a default is safer than `vae.arch` directly in case a test passes a dict-like object.
- **Handle `get_module()` returning `None`**: The dispatcher returns `None` for unregistered keys. We raise a clear `RuntimeError` — this is the correct failure mode, not a silent crash.
- **Pass `vae` (the module) and `latent` to `decode()`**: The `decode()` function signature is `decode(vae_module, latent, output_mode="RGB")`. The `vae_module` is the loaded `ZiTVaeModel` instance.
- **Return `{"image": images}`**: The `images` is a `list[PIL.Image.Image]`, matching the existing docstring which says "Dict with key 'image' containing a list of PIL.Image.Image objects".

### Step 2: Update the parity markers on `execute()`

The existing markers are:
```python
# REAL_PATH_VERIFIED: worker/tests/test_nodes_decode.py::test_vae_decode_real_decodes_zit_vae_fixture
# MOCK_PATH_VERIFIED: worker/tests/test_nodes_decode.py::test_vae_decode_mock_returns_sentinel
```

The `MOCK_PATH_VERIFIED` marker already points to an existing, passing test (`test_vae_decode_mock_returns_sentinel` in the current test file). The `REAL_PATH_VERIFIED` marker already names the test function we will create (`test_vae_decode_real_decodes_zit_vae_fixture`). No change needed to the markers — they already reference the correct test function names.

### Step 3: Add real-mode tests to `worker/tests/test_nodes_decode.py`

Add the following tests, all marked `@pytest.mark.real_mode`:

1. **`test_vae_decode_real_decodes_zit_vae_fixture`** — Primary real-mode test. Loads the ZiT VAE fixture via `arch.vae.zit_vae.load()`, creates a `(1, 4, 8, 8)` latent tensor, constructs a non-mock `NodeContext`, executes `VaeDecode.execute()`, and asserts:
   - Result is a dict with key `"image"`.
   - `"image"` is a `list` of exactly 1 `PIL.Image.Image`.
   - Image mode is `"RGB"`.
   - Image size matches latent spatial dimensions `(8, 8)`.

2. **`test_vae_decode_real_batched_latent`** — Batched latent `(2, 4, 8, 8)` produces 2 images.

3. **`test_vae_decode_real_output_rgb_uint8`** — Output PIL Image has valid uint8 pixel values in [0, 255] range.

4. **`test_vae_decode_real_arch_dispatch_uses_vae_arch`** — Verifies that `get_module()` is called with the correct `arch` key from the vae input. Uses `unittest.mock.patch` on `arch.vae.get_module` to assert the key passed is `"zit_vae"`.

5. **`test_vae_decode_real_missing_arch_raises`** — Pass a dict-like vae input without `.arch` attribute and assert `ValueError` is raised.

6. **`test_vae_decode_real_unregistered_arch_raises`** — Patch `arch.vae.get_module` to return `None` for the given arch key, and assert `RuntimeError` is raised with a descriptive message.

7. **`test_vae_decode_real_missing_vae_input_raises`** — Call execute without the `"vae"` input and assert `ValueError` is raised.

These 7 new tests satisfy the `>=5 new tests` requirement and bring the total to 10 (3 existing + 7 new), exceeding the `>=8 total` acceptance criterion.

### Step 4: Update the docstring on `execute()`

Update the `Raises` section of the docstring to remove the stale `NotImplementedError` entry and add the real error types:

```
Raises:
    ValueError: When 'vae' or 'latent' inputs are missing or None, or when
        the vae input lacks an .arch attribute.
    RuntimeError: When no registered VAE module handles the vae's arch key,
        or when torch is not installed in the decode() call.
```

## Public API Surface

| Item | Location | Description |
|------|----------|-------------|
| `VaeDecode.execute()` | `worker/nodes/decode.py` | Modified: real branch now dispatches to `arch.vae.get_module().decode()` instead of raising `NotImplementedError`. No signature change. |

No new public items are introduced. The `VaeDecode` class already exists; only the `execute()` method body is modified.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/decode.py` | Replace `NotImplementedError` stub with real dispatch to `arch.vae.get_module(vae.arch).decode(vae, latent)`; update docstring; markers already correct. |
| MODIFY | `worker/tests/test_nodes_decode.py` | Add >=5 new real-mode tests (7 planned) for VaeDecode's real branch. |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `test_nodes_decode.py` | `test_vae_decode_real_decodes_zit_vae_fixture (real)` | End-to-end: load ZiT VAE fixture, create latent, execute VaeDecode, assert real PIL Image output (mode RGB, size 8x8). Satisfies `REAL_PATH_VERIFIED` marker. | `python -m pytest worker/tests/test_nodes_decode.py::test_vae_decode_real_decodes_zit_vae_fixture -v` exits 0 |
| `test_nodes_decode.py` | `test_vae_decode_real_batched_latent (real)` | Batched latent (batch=2) produces exactly 2 PIL Images. | `python -m pytest worker/tests/test_nodes_decode.py::test_vae_decode_real_batched_latent -v` exits 0 |
| `test_nodes_decode.py` | `test_vae_decode_real_output_rgb_uint8 (real)` | Output PIL Image has valid uint8 pixel values in [0, 255]. | `python -m pytest worker/tests/test_nodes_decode.py::test_vae_decode_real_output_rgb_uint8 -v` exits 0 |
| `test_nodes_decode.py` | `test_vae_decode_real_arch_dispatch_uses_vae_arch (real)` | `arch.vae.get_module()` is called with `arch_key="zit_vae"` from the loaded VAE's `.arch` attribute. | `python -m pytest worker/tests/test_nodes_decode.py::test_vae_decode_real_arch_dispatch_uses_vae_arch -v` exits 0 |
| `test_nodes_decode.py` | `test_vae_decode_real_missing_arch_raises (real)` | Dict-like vae input without `.arch` raises `ValueError`. | `python -m pytest worker/tests/test_nodes_decode.py::test_vae_decode_real_missing_arch_raises -v` exits 0 |
| `test_nodes_decode.py` | `test_vae_decode_real_unregistered_arch_raises (real)` | `get_module()` returning `None` raises `RuntimeError` with descriptive message. | `python -m pytest worker/tests/test_nodes_decode.py::test_vae_decode_real_unregistered_arch_raises -v` exits 0 |
| `test_nodes_decode.py` | `test_vae_decode_real_missing_vae_input_raises (real)` | Missing `"vae"` input raises `ValueError`. | `python -m pytest worker/tests/test_nodes_decode.py::test_vae_decode_real_missing_vae_input_raises -v` exits 0 |
| `test_nodes_decode.py` | `test_vae_decode_mock_returns_sentinel (mock)` | Existing mock test — confirms mock path still works after real branch implementation. Satisfies `MOCK_PATH_VERIFIED` marker. | `python -m pytest worker/tests/test_nodes_decode.py::test_vae_decode_mock_returns_sentinel -v` exits 0 |

Acceptance command for the full task:
```bash
python -m pytest worker/tests/test_nodes_decode.py -v -m real_mode
# -> >=8 total tests in file, exits 0
```

## CI Impact

No CI changes required. The new tests are marked `@pytest.mark.real_mode` and are already picked up by the existing `worker-linux-real` and `worker-windows-real` CI jobs (`python -m pytest worker/tests -v -m real_mode`). The mock-mode tests remain unchanged. No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The implementation uses only Python standard library and already-imported packages (torch, PIL, numpy) — all cross-platform. The `arch.vae.get_module()` dispatcher and `decode()` function are platform-neutral.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `arch.vae.get_module()` returns `None` for `"zit_vae"` if the dispatcher's `_REGISTERED_MODULES` list is empty or the module import fails. | Low | High | The dispatcher eagerly imports `zit_vae` at module load time (see `arch/vae/__init__.py` line 20-23). If the import fails, the entire dispatcher would fail at import time, which is a clear failure mode. The `get_module()` returning `None` path is for an *unregistered* arch key, not for the zit_vae key. |
| `decode()` returns an empty list for edge-case latent shapes, causing downstream tests to pass vacuously. | Low | Medium | Tests assert `len(images) == 1` or `len(images) == 2` (non-zero batch counts), so empty list is caught. |
| The fixture's decoded output might not be a valid PIL Image if the VAE forward pass produces NaN/Inf values. | Very Low | High | The fixture uses random tensors (torch.randn) and the VAE's SiLU activation is bounded. The `decode()` function clamps to [0, 1] float before PIL conversion. If NaN occurs, the clamp step would produce NaN in the uint8 array, which PIL would reject — the test would fail with a clear error. |
| `NotImplementedError` stub's `defers_to: P24-B2` comment remains in the file after replacement, creating a false marker. | Low | Medium | The plan explicitly replaces the entire else branch including the `defers_to` comment. The ACT agent should verify the comment is removed. |

## Acceptance Criteria

- [ ] `python -m py_compile worker/nodes/decode.py` exits 0
- [ ] `python -m py_compile worker/tests/test_nodes_decode.py` exits 0
- [ ] `python -m pytest worker/tests/test_nodes_decode.py::test_vae_decode_mock_returns_sentinel -v` exits 0 (mock path unchanged)
- [ ] `python -m pytest worker/tests/test_nodes_decode.py::test_vae_decode_class_attributes -v` exits 0 (class attributes unchanged)
- [ ] `python -m pytest worker/tests/test_nodes_decode.py::test_vae_decode_in_registry -v` exits 0 (registration unchanged)
- [ ] `python -m pytest worker/tests/test_nodes_decode.py -v -m real_mode` exits 0 with >= 8 total tests in the file
- [ ] `grep "REAL_PATH_VERIFIED: worker/tests/test_nodes_decode.py::test_vae_decode_real_decodes_zit_vae_fixture" worker/nodes/decode.py` returns 0 matches (marker present)
- [ ] `grep "MOCK_PATH_VERIFIED: worker/tests/test_nodes_decode.py::test_vae_decode_mock_returns_sentinel" worker/nodes/decode.py` returns 0 matches (marker present)
- [ ] `grep -rn "defers_to: P24-B2" worker/nodes/decode.py` returns no matches (stub comment removed)
- [ ] `grep -rn "NotImplementedError" worker/nodes/decode.py` returns no matches (no stub remains)
