# Plan Report: P14-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P14-A1                                      |
| Phase       | 014 — Artifact Storage                      |
| Description | worker: mock SaveImage emits ImageReady with black PNG |
| Depends on  | P13-A6                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-06-09T14:25:00Z                        |
| Attempt     | 1                                           |

## Objective

Extend the mock `Execute` handler in `worker/worker_main.py` (`_execute_mock`) so that when a node of type `SaveImage` is encountered during graph execution, it generates a black 64×64 PNG image, encodes it as base64, and emits an `ImageReady` event (with fields: `job_id`, `image_b64`, `width:64`, `height:64`, `format:'png'`, `seed`, `steps`, `prompt`) before the final `Completed` event.

## Scope

### In Scope
- Modify `_execute_mock()` in `worker/worker_main.py` to detect `SaveImage` nodes
- Generate a black PNG (64×64 RGB) using `PIL.Image.new` and `io.BytesIO`
- Base64-encode the PNG bytes
- Emit `ImageReady{job_id, image_b64, width:64, height:64, format:'png', seed, steps, prompt}` before `Completed`
- Resolve `prompt`, `seed`, and `steps` from the SaveImage node's `inputs` dict; fall back to defaults when absent
- Handle `seed == -1` by generating a random integer in range [0, 2^63-1]
- Add `import random` and `import base64` at the top of `worker_main.py` (standard library, no new deps)

### Out of Scope
- Real node execution (handled in Phase 21: `worker/nodes/common.py SaveImage`)
- Any Rust-side changes (no crate version bumps needed)
- Any test file changes (test will be added in P14-A4 or P14-A5 as per design)
- Artifact store, server handlers, scheduler ImageReady handling (P14-A2 through P14-A5)
- Topological sort of the graph (already validated by server; nodes executed in given order)

## Approach

1. **Add imports** at the top of `worker_main.py` (after existing imports, before `_execute_mock`):
   - `import base64`
   - `import random`
   These are Python standard library modules — no new dependencies.

2. **Add a helper function** `_generate_black_png()` after `_probe_hardware()` and before `_execute_mock()`:
   ```python
   def _generate_black_png() -> bytes:
       """Create a 64x64 black RGB PNG and return raw PNG bytes."""
       from io import BytesIO
       from PIL import Image
       img = Image.new("RGB", (64, 64), (0, 0, 0))
       buf = BytesIO()
       img.save(buf, format="PNG")
       return buf.getvalue()
   ```
   This uses `Pillow>=10.0` which is already declared in `worker/requirements/base.txt`.

3. **Modify `_execute_mock()`** to detect SaveImage nodes:
   - Iterate over `graph['nodes']` as before, emitting `Progress` for each node.
   - When `node_type == "SaveImage"`:
     a. Extract `inputs` from the node: `node.get("inputs", {})`
     b. Resolve `prompt`: `inputs.get("prompt", "")` (default empty string)
     c. Resolve `seed`: `inputs.get("seed", settings.get("seed", -1))` — if -1, replace with `random.randint(0, 2**63 - 1)`
     d. Resolve `steps`: `inputs.get("steps", settings.get("steps", 1))`
     e. Generate PNG bytes via `_generate_black_png()`
     f. Base64-encode: `base64.b64encode(png_bytes).decode("ascii")`
     g. Emit `ImageReady` event via `ipc.write_frame()` with all required fields
   - Continue iterating remaining nodes (there may be multiple SaveImage nodes in a graph)

4. **Default values** when no inputs are present:
   - `prompt`: `""` (empty string)
   - `seed`: resolve from `settings["seed"]` if present, else `-1` → random
   - `steps`: from `settings["steps"]` if present, else `1`

5. **Event emission order**: For each SaveImage node, emit `Progress` then `ImageReady` immediately. After all nodes, emit `Completed` as before.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/worker_main.py` | Add imports, `_generate_black_png()` helper, and SaveImage detection in `_execute_mock()` |

No Rust crate version bumps needed (Python-only change).

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `worker/tests/test_worker_main.py` | (new test) `test_execute_saveimage_imageready` | Execute with a graph containing a SaveImage node → verify ImageReady event emitted with correct fields (width=64, height=64, format='png', valid base64 image_b64, resolved seed, steps, prompt), followed by Completed |
| `worker/tests/test_worker_main.py` | (new test) `test_execute_saveimage_seed_resolution` | Execute with SaveImage node having `seed: -1` → verify ImageReady seed is a valid random int in range |
| `worker/tests/test_worker_main.py` | (new test) `test_execute_saveimage_inputs_resolved` | Execute with SaveImage node having explicit prompt/seed/steps inputs → verify ImageReady fields match node inputs |
| `worker/tests/test_worker_main.py` | (new test) `test_execute_no_saveimage_no_imageready` | Execute with a graph that has no SaveImage node → verify NO ImageReady event emitted, only Progress + Completed |

## CI Impact

No CI workflow changes required. The Python worker tests are already gated under `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v` in CI (both Linux and Windows runners per Phase 009). The `pillow` dependency is already installed in the CI test venv (`pip install msgpack pillow pytest`). The new tests will be discovered automatically by pytest.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `PIL` import fails in mock mode if Pillow is not installed in test venv | Low | Tests fail, CI breaks | Pillow is already in `worker/requirements/base.txt` and installed in CI test venv (verified by Phase 009 CI setup) |
| Base64-encoded PNG exceeds 64 MiB IPC payload limit | Negligible | Frame rejected by Rust server | A 64×64 PNG is ~128 bytes raw; base64-encoded ≈ 170 bytes — far under the 64 MiB cap |
| Seed resolution conflicts with existing node execution logic | Low | Wrong seed value in ImageReady | Defaults follow the design spec (§4.1, §14.6): node inputs authoritative, settings fallback, -1 → random |
| Existing `test_execute_progress_completed` test breaks due to new ImageReady emission | Medium | Test failure | The existing test uses nodes with types `LoadModel`, `Inference`, `SaveOutput` — none named `SaveImage`, so no ImageReady will be emitted. But the test should still be updated to assert zero ImageReady events to be safe |
| `random.randint(0, 2**63-1)` produces a non-reproducible seed in tests | Medium | Test flakiness if seed value is asserted | Tests should verify seed is in the valid range rather than checking an exact value |

## Acceptance Criteria

- [ ] `_execute_mock()` emits `ImageReady` when a `SaveImage` node is encountered in the graph
- [ ] `ImageReady` fields: `width=64`, `height=64`, `format='png'`, valid base64 `image_b64`, resolved `seed`, `steps`, `prompt`
- [ ] Seed resolution: `seed == -1` → random int in [0, 2^63-1]; explicit seed value preserved
- [ ] Prompt and steps resolved from node inputs, falling back to settings, then defaults
- [ ] Existing `test_execute_progress_completed` test still passes (no SaveImage node → no ImageReady)
- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v` exits 0 (all existing + new tests pass)
