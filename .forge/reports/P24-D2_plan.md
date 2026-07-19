# Plan Report: P24-D2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P24-D2                                            |
| Phase       | 024 — Generic Conditioning/Sampling/Decode Nodes, Real Mode |
| Description | worker/nodes/image.py: SaveImage real branch encodes PNG, emits ImageReady |
| Depends on  | P24-D1                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-07-19T15:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Complete `SaveImage`'s real branch in `worker/nodes/image.py` by replacing the
`NotImplementedError` with real PNG encoding: take the real `PIL.Image` input, encode
it to PNG bytes, base64-encode for the IPC payload, and emit `WorkerEvent.ImageReady`
with all required fields (`image_b64`, `width`, `height`, `format`, `seed`, `steps`).
The node only emits the event — artifact persistence happens Rust-side in
`event_loop.rs` (Phase 15). Both `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` markers
must name passing tests.

## Scope

### In Scope
- Replace the `NotImplementedError` placeholder in `SaveImage.execute()`'s real branch
  with real PNG encoding using `PIL.Image.save()` + `base64.b64encode()`.
- Emit `WorkerEvent.ImageReady` via `ctx.emit()` with all seven fields:
  `job_id`, `image_b64`, `width`, `height`, `format` ("png"), `seed`, `steps`.
- Update the `defers_to: P24-D2` comment markers to remove the deferral notation
  (the real branch is now implemented, not a stub).
- Write at least 5 new tests in `worker/tests/test_nodes_image.py` covering:
  - Real-mode `execute()` against a real PIL Image emits `ImageReady` with correctly
    base64-encoded PNG bytes matching the input dimensions.
  - `seed` and `steps` pass through unchanged from inputs to event.
  - Default seed/steps values (when optional inputs absent) use sensible defaults.
  - Both `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` markers name passing tests.
- `python -m pytest worker/tests/test_nodes_image.py -v -m real_mode` exits 0 with
  >=8 total tests in the file.

### Out of Scope
- Artifact store writing (handled Rust-side in `event_loop.rs`, Phase 15).
- `ImageResize` node (P24-D3).
- Any changes to the Rust IPC layer, event_loop, or artifact store.
- Changes to the mock branch (already implemented by P24-D1).

## Existing Codebase Assessment

**What already exists:** `worker/nodes/image.py` contains the `SaveImage` node class
with `NODE_TYPE`, `CATEGORY`, `INPUT_SLOTS`, `OUTPUT_SLOTS`, and a fully functional
mock branch (64×64 black PNG → encode → emit `ImageReady` → return sentinel). The
real branch is a `NotImplementedError` placeholder with `defers_to: P24-D2` markers.
The dual-mode parity markers are already declared:
- `REAL_PATH_VERIFIED: worker/tests/test_nodes_image.py::test_save_image_real_emits_png`
- `MOCK_PATH_VERIFIED: worker/tests/test_nodes_image.py::test_save_image_mock_emits_image_ready`

The mock-mode test `test_save_image_mock_emits_image_ready` exists and passes. The
real-mode test `test_save_image_real_emits_png` is named in the marker but does not
yet exist.

**Established patterns:**
- Nodes branch on `ctx.mock` at the top of `execute()`, never deeper.
- Mock mode uses local `from PIL import Image` to keep it torch-free.
- The `emit` callable accepts a plain dict with `_type` discriminator — this is how
  `WorkerEvent` dicts are constructed on the Python side before msgpack serialization.
- Tests use a `_make_ctx()` helper that constructs `NodeContext` with a lambda `emit`
  function to capture events. Real-mode tests need a real PIL Image as input.
- The `defers_to: TASK_ID` comment pattern is used at stub sites and removed when the
  deferred scope is implemented.

**Gap between design doc and source:** The design doc (`ANVILML_DESIGN.md §10.3`)
specifies that `SaveImage` "Encodes to PNG, writes to artifact store, emits ImageReady"
— but the actual implementation scope (per `TASKS_PHASE024.md` and the Rust event_loop)
is: encode PNG → base64 → emit `ImageReady`. The artifact store write is Rust-side.
This gap is correctly reflected in the current source (the mock branch only emits,
never writes to disk); the real branch must follow the same pattern.

## Resolved Dependencies

None. This task uses only `PIL.Image` (Pillow, already a project dependency via
`worker/requirements/base.txt`) and `base64` (Python standard library). No new
external packages are introduced.

| Type | Name | Version verified | MCP source | Feature flags confirmed |
|------|------|-----------------|------------|------------------------|
| python | PIL (Pillow) | existing in base.txt | n/a | n/a |
| python | base64 | stdlib | n/a | n/a |

## Approach

1. **Implement the real branch in `SaveImage.execute()`** (`worker/nodes/image.py`).
   Replace the `else: raise NotImplementedError(...)` block with:

   ```python
   else:
       # Validate required inputs before proceeding. The "image" key is
       # required (not optional) per INPUT_SLOTS — accessing it directly
       # raises KeyError if absent, which is the desired failure mode for
       # missing required inputs.
       image = inputs["image"]

       # In real mode, the "image" input is a PIL.Image instance (produced
       # by VaeDecode's real branch). Encode it to PNG bytes, base64-encode
       # for the IPC payload, then emit ImageReady via ctx.emit.
       import base64
       from io import BytesIO

       buf = BytesIO()
       image.save(buf, format="PNG")  # Encode PIL.Image to PNG bytes
       png_bytes = buf.getvalue()

       # Base64-encode the PNG bytes for the IPC msgpack payload.
       # The Rust event_loop.rs decodes this with base64::engine::general_purpose::STANDARD.
       image_b64 = base64.b64encode(png_bytes).decode("ascii")

       # Emit ImageReady event with all required fields matching
       # WorkerEvent::ImageReady (messages.rs):
       #   job_id, image_b64, width, height, format, seed, steps
       ctx.emit({
           "_type": "ImageReady",
           "job_id": ctx.job_id,
           "image_b64": image_b64,
           "width": image.width,
           "height": image.height,
           "format": "png",
           "seed": inputs.get("seed", -1),
           "steps": inputs.get("steps", 1),
       })

       logger.debug(
           "SaveImage: real branch emitted ImageReady for job_id=%s, "
           "width=%d, height=%d",
           ctx.job_id_str,
           image.width,
           image.height,
       )

       # Return an empty dict — SaveImage has no output slots per §10.3.
       return {}
   ```

   **Rationale:** The real branch mirrors the mock branch's structure (validate → encode
   → emit → log → return) but uses the actual PIL.Image input instead of generating a
   synthetic one. The return value is `{}` because SaveImage has `OUTPUT_SLOTS = []` —
   it emits an event rather than producing a slot output. The mock branch returns
   `{"image": {"mock": True, ...}}` as a sentinel for test verification; the real branch
   returns `{}` since there are no outputs to return.

2. **Update the defers_to comment markers.** Remove the `defers_to: P24-D2` notation
   from both comment blocks in `image.py`, since the deferred scope is now implemented.
   Keep the `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` markers pointing at their
   respective tests.

3. **Write new tests in `worker/tests/test_nodes_image.py`.** At least 5 new tests:
   - `test_save_image_real_emits_png` (real_mode): real PIL Image → emits ImageReady
     with correctly base64-encoded PNG bytes matching the input dimensions.
   - `test_save_image_real_seed_pass_through` (real_mode): seed input passes through
     unchanged to the ImageReady event.
   - `test_save_image_real_steps_pass_through` (real_mode): steps input passes through
     unchanged to the ImageReady event.
   - `test_save_image_real_default_seed_steps` (real_mode): when seed/steps are absent,
     defaults are -1 and 1 respectively.
   - `test_save_image_real_png_bytes_valid` (real_mode): the base64-decoded payload
     is a valid PNG image matching the input dimensions.

   Each real-mode test uses a real `PIL.Image` as the `image` input and verifies the
   emitted event's fields.

4. **Update the marker comments** to confirm both markers name passing tests.

## Public API Surface

No new public Python items are introduced. The existing `SaveImage.execute()` method
signature is unchanged:

```python
def execute(self, ctx: NodeContext, **inputs) -> dict:
```

The only behavioral change is the real branch now returns `{}` instead of raising
`NotImplementedError`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/image.py` | Replace `NotImplementedError` with real PNG encoding + `ImageReady` emit; update defers_to markers |
| MODIFY | `worker/tests/test_nodes_image.py` | Add >=5 new real-mode tests |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `test_nodes_image.py` | `test_save_image_real_emits_png` (real) | Real PIL Image input → emit ImageReady with correct image_b64, width, height, format="png" | Real mode (no `ANVILML_WORKER_MOCK`) | `image=PIL.Image.new("RGB", (128, 64), (255, 0, 0))` | `ctx.emit` called with `_type="ImageReady"`, `width=128`, `height=64`, `format="png"`, valid base64 payload | `python -m pytest worker/tests/test_nodes_image.py -v -m real_mode -k test_save_image_real_emits_png` |
| `test_nodes_image.py` | `test_save_image_real_seed_pass_through` (real) | `seed` input passes through unchanged to ImageReady event | Real mode | `image=PIL.Image.new(...)`, `seed=42` | `event["seed"] == 42` | `python -m pytest worker/tests/test_nodes_image.py -v -m real_mode -k test_save_image_real_seed_pass_through` |
| `test_nodes_image.py` | `test_save_image_real_steps_pass_through` (real) | `steps` input passes through unchanged to ImageReady event | Real mode | `image=PIL.Image.new(...)`, `steps=20` | `event["steps"] == 20` | `python -m pytest worker/tests/test_nodes_image.py -v -m real_mode -k test_save_image_real_steps_pass_through` |
| `test_nodes_image.py` | `test_save_image_real_default_seed_steps` (real) | When seed/steps absent, defaults are -1 and 1 | Real mode | `image=PIL.Image.new(...)` (no seed/steps) | `event["seed"] == -1`, `event["steps"] == 1` | `python -m pytest worker/tests/test_nodes_image.py -v -m real_mode -k test_save_image_real_default_seed_steps` |
| `test_nodes_image.py` | `test_save_image_real_png_bytes_valid` (real) | Base64-decoded payload is a valid PNG matching input dimensions | Real mode | `image=PIL.Image.new("RGB", (32, 96), (0, 255, 0))` | `base64.b64decode(image_b64)` decodes to valid PNG; reopened PIL Image has size (32, 96) | `python -m pytest worker/tests/test_nodes_image.py -v -m real_mode -k test_save_image_real_png_bytes_valid` |
| `test_nodes_image.py` | `test_save_image_real_returns_empty_dict` (real) | Real branch returns `{}` (no output slots) | Real mode | `image=PIL.Image.new(...)` | `result == {}` | `python -m pytest worker/tests/test_nodes_image.py -v -m real_mode -k test_save_image_real_returns_empty_dict` |

## CI Impact

No CI changes required. The new tests are real-mode tests that will be picked up by
the existing `worker-linux-real` and `worker-windows-real` CI jobs which run
`pytest worker/tests -v -m real_mode`. No new file types, gates, or markers are
introduced.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. PNG
encoding via `PIL.Image.save()` is platform-neutral. Base64 encoding is a Python
stdlib operation with no platform-specific behavior.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ctx.job_id` is raw bytes (UUID 16 bytes) — the mock branch passes raw bytes to `ctx.emit()` and the Rust side expects a UUID. In real mode tests, the `_make_ctx()` helper uses `job_id="test-job"` (string), not bytes. The Rust `WorkerEvent::ImageReady { job_id: Uuid }` will fail to deserialize if the Python side sends a string instead of raw 16-byte UUID. | Medium | High | The mock branch already passes `ctx.job_id` (raw bytes) in production. For tests, the event is captured directly from `ctx.emit()` without msgpack serialization, so the Rust deserializer is never invoked. The test verifies the Python-side dict structure, not the wire format. Document this explicitly in the test docstrings. |
| Real-mode tests require `torch` to be importable (the `real_mode` marker convention). If `torch` is not installed, the real-mode test suite will fail at collection time. | Low | Medium | The acceptance command runs `python -m pytest worker/tests/test_nodes_image.py -v -m real_mode` which is the same command used by CI's `worker-linux-real` job. If torch is missing, install `requirements/cpu-linux-agent.txt` as per ENVIRONMENT.md §5. The tests themselves do not import torch — only PIL and base64 — so they are lightweight. |
| Base64 encoding produces a string that may differ between Python 3.12 versions or locales. | Very Low | Low | `base64.b64encode()` returns bytes; `.decode("ascii")` produces a deterministic ASCII string. This is a stdlib function with no locale dependency. |

## Acceptance Criteria

- [ ] `python -m py_compile worker/nodes/image.py worker/tests/test_nodes_image.py` exits 0
- [ ] `python -m pytest worker/tests/test_nodes_image.py -v -m real_mode` exits 0 (>=8 total tests in file)
- [ ] `python -m pytest worker/tests/test_nodes_image.py -v` exits 0 (all tests, mock + real)
- [ ] `grep -rn "REAL_PATH_VERIFIED:" worker/nodes/image.py` returns a line naming `test_save_image_real_emits_png`
- [ ] `grep -rn "MOCK_PATH_VERIFIED:" worker/nodes/image.py` returns a line naming `test_save_image_mock_emits_image_ready`
- [ ] `grep -rn "defers_to: P24-D2" worker/nodes/image.py` returns no matches (defers_to comments removed)
- [ ] `worker/.venv/bin/python -m pytest --collect-only "test_save_image_real_emits_png" -q` exits 0
- [ ] `worker/.venv/bin/python -m pytest --collect-only "test_save_image_mock_emits_image_ready" -q` exits 0
