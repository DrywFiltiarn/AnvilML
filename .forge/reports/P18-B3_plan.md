# Plan Report: P18-B3

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P18-B3                                            |
| Phase       | 018 — ZiT Generic Nodes                           |
| Description | worker/nodes/decode.py: VaeDecode node with explicit VAE input |
| Depends on  | P18-A2 (LoadVae), P18-B2 (Sampler)                |
| Project     | anvilml                                           |
| Planned at  | 2026-06-21T14:50:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create `worker/nodes/decode.py` containing the `VaeDecode` node class and its `MockImage` sentinel, registered in `NODE_REGISTRY` via `@register`. The node accepts explicit `vae:VAE` and `latent:LATENT` inputs and returns `image:IMAGE`. In mock mode, it returns `{"image": MockImage()}`. Also update `worker/nodes/image.py`'s `SaveImage` node to include `seed` and `steps` in its `ImageReady` event emission (matching the real-mode contract). Create `worker/tests/test_nodes_decode.py` with ≥3 tests.

## Scope

### In Scope
- Create `worker/nodes/decode.py` with:
  - `MockImage` sentinel class (lightweight placeholder, no image data)
  - `VaeDecode` node class decorated with `@register`, `INPUT_SLOTS=[SlotSpec("vae","VAE"), SlotSpec("latent","LATENT")]`, `OUTPUT_SLOTS=[SlotSpec("image","IMAGE")]`
  - Mock-mode `execute()` returning `{"image": MockImage()}`
  - Real-mode stub raising `NotImplementedError`
  - Module-level docstring, class docstrings, inline comments per project conventions
- Update `worker/nodes/image.py` `SaveImage.execute()` to accept optional `seed` and `steps` inputs and include them in the `ImageReady` event dict (matching the `WorkerEvent.ImageReady` contract from `ANVILML_DESIGN.md §5.8` and `§8.5`)
- Create `worker/tests/test_nodes_decode.py` with ≥3 tests covering registry registration, mock execution, and metadata attributes

### Out of Scope
- Real VAE decoding implementation (torch/diffusers/safetensors imports)
- `VaeEncode` node (future phase)
- `ImageResize` node implementation
- Architecture dispatch for VAE decoding (`arch/zit.py` handles Sampler, not decode)
- Any Rust-side changes

## Existing Codebase Assessment

The Python worker node system follows a strict pattern established in Phase 017. `worker/nodes/base.py` defines `BaseNode` (ABC with `execute(**inputs)`), `NodeContext` (runtime context with `job_id`, `device`, `cancel_flag`, `emit`, `pipeline_cache`), `SlotSpec` (dataclass for slot declarations), and the `@register` decorator that validates six metadata attributes and stores the class in the global `NODE_REGISTRY`.

Existing node modules (`loader.py`, `encoder.py`) all follow the same structure: a module-level docstring, mock sentinel classes (e.g. `MockModel`, `MockVae`, `MockClip`, `MockConditioning`), a `@register`-decorated node class with metadata attributes, and an `execute()` method that checks `os.environ.get("ANVILML_WORKER_MOCK") == "1"` to dispatch between mock and real paths. The real path is stubbed with `NotImplementedError` and a `TODO` comment referencing the task ID.

No `MockImage` class exists anywhere in the codebase — it must be created as part of this task. The `SaveImage` node in `image.py` currently emits `ImageReady` with `job_id`, `image_b64`, `width`, and `height` but does not include `seed` or `steps` fields that appear in the `WorkerEvent.ImageReady` variant (`ANVILML_DESIGN.md §8.5`). The test pattern uses `registry_clean` and `mock_context` fixtures, `importlib.reload()` for module re-import, and three categories of tests: registry registration, mock execution, and metadata attribute verification.

## Resolved Dependencies

None. This task introduces no new external Python packages. It uses only the Python standard library (`os`, `base64`, `io`, `struct`, `zlib`) and existing project modules (`worker.nodes.base`, `worker.nodes`).

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| (none) | (none)  | (n/a)           | (n/a)          | (n/a)                  |

## Approach

1. **Create `worker/nodes/decode.py`** with the following structure:

   a. Module docstring explaining the VaeDecode node's purpose, the mock-mode sentinel, and the real-mode stub. Include the standard import guard note about not importing torch/diffusers/safetensors at module level.

   b. Define `MockImage` class — a lightweight sentinel with no fields (matching the pattern of `MockVae` in `loader.py`). Add Google-style docstring.

   c. Define `VaeDecode` class decorated with `@register`:
      - `NODE_TYPE = "VaeDecode"`
      - `CATEGORY = "Decoding"`
      - `DISPLAY_NAME = "VAE Decode"`
      - `DESCRIPTION = "Decode a latent tensor to an image using a VAE"`
      - `INPUT_SLOTS = [SlotSpec("vae", "VAE"), SlotSpec("latent", "LATENT")]`
      - `OUTPUT_SLOTS = [SlotSpec("image", "IMAGE")]`
      - `execute(**inputs)` method that:
        - Reads `vae` and `latent` inputs (both required, but mock mode ignores them)
        - Checks `os.environ.get("ANVILML_WORKER_MOCK") == "1"`
        - In mock mode: returns `{"image": MockImage()}`
        - In real mode: stub with `NotImplementedError("Real VaeDecode path not yet implemented — use ANVILML_WORKER_MOCK=1 for testing")`
      - Add Google-style docstrings on the class and method

   d. `__all__ = ["VaeDecode", "MockImage"]`

2. **Update `worker/nodes/image.py`** `SaveImage.execute()`:

   a. Read `seed` and `steps` from inputs: `seed = inputs.get("seed")` and `steps = inputs.get("steps")`. These are optional — the node's `INPUT_SLOTS` only declares `"image"` as required, so `seed` and `steps` arrive only when wired from upstream nodes (per the Appendix B workflow, `SaveImage` receives `seed` from `Sampler` and `steps` is an optional input).

   b. Include `seed` and `steps` in the `ImageReady` event dict. The existing event includes `job_id`, `image_b64`, `width`, `height`. Add `seed` (from `inputs.get("seed")`, defaulting to `None`) and `steps` (from `inputs.get("steps")`, defaulting to `None`). This matches the `WorkerEvent.ImageReady` schema in `ANVILML_DESIGN.md §8.5` which declares `seed: i64, steps: u32` as fields.

   c. The `INPUT_SLOTS` on `SaveImage` stays as `[SlotSpec("image", "IMAGE")]` — `seed` and `steps` are accepted via `**inputs` but not declared as formal slot specs. This is consistent with how optional node inputs work in the existing codebase (e.g., `ClipTextEncode` has `negative_text` as an optional slot, but `SaveImage`'s `seed`/`steps` are accepted through the graph wiring mechanism).

3. **Create `worker/tests/test_nodes_decode.py`** with the following tests:

   a. `test_vaedeode_registered_in_registry()` — Clear `NODE_REGISTRY`, re-import and reload `worker.nodes.decode`, assert `"VaeDecode"` is in `NODE_REGISTRY` and `NODE_REGISTRY["VaeDecode"] is VaeDecode`.

   b. `test_vaedeode_execute_returns_mock_image()` — Instantiate `VaeDecode(mock_context)`, call `execute(vae=MockVae(), latent=MockLatent())`, assert `"image"` in result and `isinstance(result["image"], MockImage)`. Uses `MockVae` imported from `worker.nodes.loader` (the established mock VAE sentinel).

   c. `test_vaedeode_metadata_attributes()` — Assert all six metadata attributes: `NODE_TYPE == "VaeDecode"`, `CATEGORY == "Decoding"`, `DISPLAY_NAME == "VAE Decode"`, `DESCRIPTION` is non-empty string, `INPUT_SLOTS` has two specs (`vae:VAE` required, `latent:LATENT` required), `OUTPUT_SLOTS` has one spec (`image:IMAGE` required).

   d. `test_vaedeode_execute_missing_inputs_returns_mock()` — Call `execute()` without any inputs (matching how `LoadModel` handles missing `model_id`). Mock mode ignores inputs entirely, so result should still be `{"image": MockImage()}`.

   This gives 4 tests, exceeding the ≥3 requirement.

4. **No Rust changes needed** — this task only touches Python worker code. The `WorkerEvent.ImageReady` schema in Rust already includes `seed` and `steps` fields, so the SaveImage update is purely adding fields to the dict (backward compatible with the Rust side which already handles `Option` fields).

## Public API Surface

| Module Path | Item | Kind | Signature / Definition |
|-------------|------|------|----------------------|
| `worker.nodes.decode` | `MockImage` | Class | `class MockImage: pass` — lightweight sentinel, no fields |
| `worker.nodes.decode` | `VaeDecode` | Class (decorated with `@register`) | `class VaeDecode(BaseNode): NODE_TYPE="VaeDecode", CATEGORY="Decoding", DISPLAY_NAME="VAE Decode", DESCRIPTION="Decode a latent tensor to an image using a VAE", INPUT_SLOTS=[SlotSpec("vae","VAE"), SlotSpec("latent","LATENT")], OUTPUT_SLOTS=[SlotSpec("image","IMAGE")]` |
| `worker.nodes.decode` | `VaeDecode.execute` | Method | `def execute(self, **inputs: Any) -> dict[str, Any]` — returns `{"image": MockImage()}` in mock mode |
| `worker.nodes.image` | `SaveImage.execute` | Method (modified) | Before: emits `ImageReady` with `{job_id, image_b64, width, height}`. After: also includes `{seed, steps}` from inputs (both default `None`) |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/decode.py` | VaeDecode node + MockImage sentinel |
| MODIFY | `worker/nodes/image.py` | SaveImage: include seed/steps in ImageReady event |
| CREATE | `worker/tests/test_nodes_decode.py` | Unit tests for VaeDecode (≥4 tests) |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_nodes_decode.py` | `test_vaedeode_registered_in_registry` | VaeDecode is registered in NODE_REGISTRY after importing | NODE_REGISTRY cleared by `registry_clean` fixture; `worker.nodes.decode` reloaded | None | `"VaeDecode" in NODE_REGISTRY`, `NODE_REGISTRY["VaeDecode"] is VaeDecode`, `VaeDecode.NODE_TYPE == "VaeDecode"` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_decode.py::test_vaedeode_registered_in_registry -v` exits 0 |
| `worker/tests/test_nodes_decode.py` | `test_vaedeode_execute_returns_mock_image` | execute() returns MockImage sentinel in mock mode | `ANVILML_WORKER_MOCK=1` set by `conftest.py`; NODE_REGISTRY cleared | `vae=MockVae()`, `latent=MockLatent()` | `result["image"]` is `MockImage` instance | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_decode.py::test_vaedeode_execute_returns_mock_image -v` exits 0 |
| `worker/tests/test_nodes_decode.py` | `test_vaedeode_metadata_attributes` | All six required metadata attributes on VaeDecode are correct | VaeDecode class accessible via import | None | NODE_TYPE="VaeDecode", CATEGORY="Decoding", DISPLAY_NAME="VAE Decode", DESCRIPTION non-empty, INPUT_SLOTS=[vae:VAE, latent:LATENT], OUTPUT_SLOTS=[image:IMAGE] | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_decode.py::test_vaedeode_metadata_attributes -v` exits 0 |
| `worker/tests/test_nodes_decode.py` | `test_vaedeode_execute_missing_inputs_returns_mock` | execute() handles missing inputs gracefully in mock mode | `ANVILML_WORKER_MOCK=1` set by `conftest.py`; NODE_REGISTRY cleared | No inputs (empty dict) | `result["image"]` is `MockImage` instance | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_decode.py::test_vaedeode_execute_missing_inputs_returns_mock -v` exits 0 |

## CI Impact

No CI changes required. The new test file `worker/tests/test_nodes_decode.py` is automatically picked up by the existing `pytest worker/tests/` command in the `worker-linux` and `worker-windows` CI jobs. The Python syntax check (`py_compile`) in Step 7 will validate the new `.py` files. No new CI jobs, gates, or file patterns are introduced.

## Platform Considerations

None identified. The `VaeDecode` node and `MockImage` sentinel are pure Python with no platform-specific code paths. The mock mode code path uses only `os.environ.get()` which works identically on Linux, Windows, and macOS. The `SaveImage` update in `image.py` only adds dict keys to the `ImageReady` event, which is platform-neutral. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `MockLatent` class does not exist — `test_vaedeode_execute_returns_mock_image` passes `latent=MockLatent()` but no `MockLatent` class is defined in the codebase (it will be created in P18-B2 Sampler task). If P18-B2 has not yet been implemented, the test needs a different approach. | Medium | High | Check whether `MockLatent` exists in `worker/nodes/sampler.py` before writing the test. If not, use `None` as the latent input (mock mode ignores it) or import from `worker.nodes.loader` if `MockLatent` is defined there. The mock code path ignores both `vae` and `latent` inputs, so any sentinel or `None` works. |
| SaveImage event field changes may cause a mismatch with the Rust-side `WorkerEvent.ImageReady` deserialization if the Rust side expects exactly 4 fields. However, `WorkerEvent.ImageReady` in `ANVILML_DESIGN.md §8.5` already declares `seed: i64` and `steps: u32` fields, so adding them is correct. | Low | Low | The Rust `WorkerEvent::ImageReady` struct already has `seed` and `steps` fields (see `ANVILML_DESIGN.md §8.5`). The msgpack serialization uses flat dicts with `_type` discriminator, so extra keys in the dict are harmless — serde will deserialize only the known fields. No risk of deserialization failure. |
| Auto-import in `__init__.py` may fail silently if `decode.py` has a syntax error, masking the defect during testing. | Low | Medium | The mandatory Step 7 (`py_compile`) will catch any syntax error before pytest runs. This is the standard safety net the project has established. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/decode.py worker/nodes/image.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_decode.py -v` exits 0 with ≥3 tests passing
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v` exits 0 (full test suite, no regressions)
- [ ] `grep -c "def test_" worker/tests/test_nodes_decode.py` returns ≥ 3
