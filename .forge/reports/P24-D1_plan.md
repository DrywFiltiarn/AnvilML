# Plan Report: P24-D1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P24-D1                                      |
| Phase       | 24 — Generic Conditioning/Sampling/Decode Nodes, Real Mode |
| Description | worker/nodes/image.py: SaveImage node, mock branch only |
| Depends on  | none                                        |
| Project     | anvilml                                     |
| Planned at  | 2026-07-19T10:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `worker/nodes/image.py` with the `SaveImage` node class implementing its mock branch per `ANVILML_DESIGN.md §10.3` and `§14.6`: `NODE_TYPE="SaveImage"`, `CATEGORY="Output"`, three input slots (`image:IMAGE`, `seed:INT?`, `steps:INT?`), zero output slots, `@register` decorated. The mock `execute()` emits an `ImageReady` event via `ctx.emit` containing a 64×64 black PNG. The real branch is a bare `raise NotImplementedError` placeholder (completed by P24-D2). Produce `worker/tests/test_nodes_image.py` with ≥3 tests confirming mock emission, registry presence, and overall acceptance.

## Scope

### In Scope
- Create `worker/nodes/image.py` with `SaveImage` class:
  - `NODE_TYPE = "SaveImage"`, `CATEGORY = "Output"`
  - `INPUT_SLOTS = [SlotSpec("image", "IMAGE"), SlotSpec("seed", "INT", optional=True), SlotSpec("steps", "INT", optional=True)]`
  - `OUTPUT_SLOTS = []` (no outputs — emits event instead)
  - Mock branch: generates a 64×64 black PNG via `PIL.Image`, emits `ImageReady` event dict via `ctx.emit`
  - Real branch: bare `raise NotImplementedError("real branch not yet implemented — P24-D2")`
  - `@register` decorated
  - Dual-mode parity markers: `MOCK_PATH_VERIFIED` pointing at the mock test; `REAL_PATH_VERIFIED` placeholder pointing at a future test (P24-D2 will update this)
- Create `worker/tests/test_nodes_image.py` with ≥3 tests:
  - Mock test: `execute()` with `ctx.mock=True` emits `ImageReady` containing a 64×64 black PNG
  - Registry test: `SaveImage` appears in `NODE_REGISTRY` after importing the module
  - Input validation test: missing `image` input raises `KeyError`

### Out of Scope
- Real branch PNG encoding and artifact emission — implemented by P24-D2 (`worker/nodes/image.py`: SaveImage real branch encodes PNG, emits ImageReady). The real branch in this task is a `NotImplementedError` stub.
- `ImageResize` node — implemented by P24-D3.

## Existing Codebase Assessment

The node system is already established: `worker/nodes/base.py` provides `BaseNode` (ABC with abstract `execute()`), `NodeContext` (carrying `job_id`, `device`, `caps`, `cancel_flag`, `emit`, `pipeline_cache`, `mock`), `SlotSpec`, and the `@register` decorator that populates `NODE_REGISTRY`. The `worker/nodes/__init__.py` auto-imports all `.py` modules under `nodes/` (excluding `__init__`, `base`, and packages like `arch/`) at package load time, triggering `@register` side effects.

Existing node implementations (`loader.py`: `LoadModel`, `LoadVae`, `LoadClip`, `EmptyLatent`) follow a consistent pattern:
1. Import from `worker.nodes.base` (BaseNode, NodeContext, SlotSpec, register).
2. Define class-level attributes (`NODE_TYPE`, `CATEGORY`, `DISPLAY_NAME`, `DESCRIPTION`, `INPUT_SLOTS`, `OUTPUT_SLOTS`).
3. Place dual-mode parity markers (`REAL_PATH_VERIFIED:` / `MOCK_PATH_VERIFIED:`) as module-level comments immediately before `execute()`.
4. Branch at the top of `execute()` on `ctx.mock` — mock returns sentinel dict without any real I/O or torch imports; real dispatches to arch modules.
5. Use `logging.getLogger(__name__)` for DEBUG-level instrumentation.

Test patterns in `worker/tests/test_nodes_loader.py`:
- `_make_ctx()` helper constructs minimal `NodeContext` with `mock=True`, `job_id="test-job"`, `caps={"bf16": True, "fp8": False}`, `emit=lambda e: None`, and empty dict as pipeline_cache.
- Mock tests verify sentinel return values.
- Registry tests use `subprocess.run([sys.executable, "-c", ...])` with `timeout=10` to import the module in an isolated subprocess and assert `NODE_REGISTRY` membership.
- Real-mode tests use `@pytest.mark.real_mode` and import torch/safetensors against fixture checkpoints.

No `worker/nodes/image.py` or `worker/tests/test_nodes_image.py` exists yet. This task creates both from scratch.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| python | Pillow  | 11.x (from base.txt) | pypi-query MCP | n/a                    |

Pillow is already a dependency of the worker venv via `worker/requirements/base.txt`. No new external dependencies are introduced. The mock branch uses `PIL.Image.new("RGB", (64, 64), (0, 0, 0))` and `io.BytesIO` for PNG encoding — both standard library + Pillow, already available.

## Approach

1. **Create `worker/nodes/image.py`** with the `SaveImage` class:
   - Import `BaseNode`, `NodeContext`, `SlotSpec`, `register` from `worker.nodes.base`.
   - Import `logging` for DEBUG-level logging (`logger = logging.getLogger(__name__)`).
   - Define `SaveImage` class with:
     - `NODE_TYPE = "SaveImage"`
     - `CATEGORY = "Output"`
     - `DISPLAY_NAME = "Save Image"`
     - `DESCRIPTION = "Saves an image to the artifact store and emits ImageReady."`
     - `INPUT_SLOTS = [SlotSpec("image", "IMAGE"), SlotSpec("seed", "INT", optional=True), SlotSpec("steps", "INT", optional=True)]`
     - `OUTPUT_SLOTS = []` — SaveImage has no output slots; it emits an event instead, per §10.3's table.
   - Add dual-mode parity markers above `execute()`:
     - `REAL_PATH_VERIFIED: worker/tests/test_nodes_image.py::test_save_image_real_emits_png` (placeholder — P24-D2 will update this when the real test exists)
     - `MOCK_PATH_VERIFIED: worker/tests/test_nodes_image.py::test_save_image_mock_emits_image_ready`
   - Implement `execute(self, ctx: NodeContext, **inputs) -> dict`:
     - Branch on `ctx.mock` at the top per §14.6.
     - **Mock branch:**
       - Import `PIL.Image` and `io` locally (inside the mock branch, not at module level — keeps mock mode torch-free and dependency-free).
       - Create a 64×64 black PNG: `img = PIL.Image.new("RGB", (64, 64), (0, 0, 0))`
       - Encode to PNG bytes: buffer = `io.BytesIO()`, `img.save(buffer, format="PNG")`, `png_bytes = buffer.getvalue()`
       - Emit the `ImageReady` event dict via `ctx.emit`:
         ```python
         ctx.emit({
             "_type": "ImageReady",
             "job_id": ctx.job_id,
             "artifact_hash": "mock_black_png_64x64",
             "width": 64,
             "height": 64,
             "seed": inputs.get("seed", -1),
             "steps": inputs.get("steps", 1),
             "image_data": png_bytes.hex()[:32],  # truncated hex for test verification
         })
         ```
       - Return `{"image": {"mock": True, "width": 64, "height": 64}}` — sentinel output matching the pattern of other nodes' mock branches.
       - Log at DEBUG: `"SaveImage: mock branch emitted ImageReady for job_id=%s" % ctx.job_id_str`
     - **Real branch (placeholder):**
       - `raise NotImplementedError("real branch not yet implemented — P24-D2")`
       - This is a stub; P24-D2 will replace it with real PNG encoding + event emission.
     - Return `{"image": sentinel}` in both branches (the return value is used by the executor for downstream wiring, but SaveImage's OUTPUT_SLOTS is empty — the executor handles nodes with no outputs by simply not propagating anything downstream).

2. **Create `worker/tests/test_nodes_image.py`** with ≥3 tests:
   - **Test 1 — `test_save_image_mock_emits_image_ready`** (mock):
     - Construct `NodeContext` with `mock=True` using the `_make_ctx` pattern from `test_nodes_loader.py`.
     - Instantiate `SaveImage()`, call `execute(ctx, image={"mock": True, "width": 512, "height": 512})`.
     - Assert that `ctx.emit` was called with a dict containing `_type == "ImageReady"`, `width == 64`, `height == 64`.
     - This test exercises the mock code path and satisfies `MOCK_PATH_VERIFIED`.
   - **Test 2 — `test_save_image_in_registry`** (mock, subprocess-isolated):
     - Use `subprocess.run([sys.executable, "-c", code])` pattern from `test_nodes_loader.py`.
     - Code imports `worker.nodes.image`, checks `NODE_REGISTRY["SaveImage"]` exists and equals the imported class.
     - Assert return code 0 and "OK" in stdout.
   - **Test 3 — `test_save_image_missing_image_input_raises`** (mock):
     - Construct `NodeContext` with `mock=True`.
     - Call `execute(ctx)` without the required `image` input.
     - Assert `KeyError` is raised (Python's natural behavior when accessing a missing dict key in `**inputs`).

3. **No changes to existing files** — the `worker/nodes/__init__.py` auto-imports `image.py` by filename, so no registration changes are needed.

## Public API Surface

| Item | Module Path | Description |
|------|-------------|-------------|
| `class SaveImage(BaseNode)` | `worker.nodes.image` | Output node that emits `ImageReady` event with a 64×64 black PNG in mock mode. |
| `SaveImage.NODE_TYPE` | `worker.nodes.image` | `"SaveImage"` |
| `SaveImage.CATEGORY` | `worker.nodes.image` | `"Output"` |
| `SaveImage.INPUT_SLOTS` | `worker.nodes.image` | `[SlotSpec("image","IMAGE"), SlotSpec("seed","INT",optional=True), SlotSpec("steps","INT",optional=True)]` |
| `SaveImage.OUTPUT_SLOTS` | `worker.nodes.image` | `[]` |
| `SaveImage.execute(self, ctx, **inputs) -> dict` | `worker.nodes.image` | Mock: emits `ImageReady` with 64×64 black PNG. Real: raises `NotImplementedError`. |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/image.py` | SaveImage node class with mock branch + placeholder real branch |
| CREATE | `worker/tests/test_nodes_image.py` | Tests for SaveImage: mock emission, registry, input validation |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `worker/tests/test_nodes_image.py` | `test_save_image_mock_emits_image_ready (mock)` | `SaveImage.execute()` with `ctx.mock=True` emits `ImageReady` event dict containing a 64×64 black PNG via `ctx.emit` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_image.py::test_save_image_mock_emits_image_ready -v` |
| `worker/tests/test_nodes_image.py` | `test_save_image_in_registry (mock)` | `SaveImage` appears in `NODE_REGISTRY` after importing `worker.nodes.image` module (subprocess-isolated) | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_image.py::test_save_image_in_registry -v` |
| `worker/tests/test_nodes_image.py` | `test_save_image_missing_image_input_raises (mock)` | Calling `execute()` without required `image` input raises `KeyError` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_image.py::test_save_image_missing_image_input_raises -v` |

## CI Impact

No CI changes required. The new test file `worker/tests/test_nodes_image.py` is automatically picked up by the existing pytest command patterns (`worker/.venv/bin/python -m pytest worker/tests/ -v -m "not real_mode"` for mock-mode CI jobs). No new CI jobs, gates, or configuration changes are needed.

## Platform Considerations

None identified. The mock branch uses only `PIL.Image` and `io.BytesIO`, which are platform-neutral. No `#[cfg(unix)]` / `#[cfg(windows)]` guards required. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ctx.emit` signature mismatch — the `emit` callable in `NodeContext` may expect a specific dict shape (e.g. requiring `_type` as the first key). If the dict shape doesn't match what the Rust-side IPC handler expects, the event will be silently dropped or cause an IPC error. | Low | Medium | Review the `WorkerEvent` enum in `anvilml-ipc/src/messages.rs` to confirm the exact msgpack dict shape for `ImageReady`. The plan uses `_type: "ImageReady"` with fields matching the Rust struct's serde `rename_all = "snake_case"` — this aligns with the existing pattern for `Ready` events. |
| Dual-mode parity marker convention: this task only implements the mock branch, so the `REAL_PATH_VERIFIED` marker will point at a test that doesn't exist yet. §10.6 says every `execute()` must have both markers before the task is complete. However, the task explicitly defers the real branch to P24-D2, and the design doc (§10.6 rule 4) allows updating markers when the real branch is added later. | Low | Medium | The plan includes the `REAL_PATH_VERIFIED` marker as a placeholder. P24-D2's plan will update it. This is consistent with how `EmptyLatent`'s real-mode test was handled in the loader module — the marker existed before the real test was written. |
| Pillow availability: the mock branch imports `PIL.Image` which must be in the worker venv's `base.txt`. If Pillow is not installed, the mock branch will fail at import time. | Low | High | Pillow is already in `worker/requirements/base.txt` (confirmed by ARCHITECTURE.md §2 listing `pillow` as a core dependency). No action needed. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_image.py -v` exits 0 with ≥3 tests
- [ ] `worker/nodes/image.py` exists and defines `SaveImage` with `NODE_TYPE = "SaveImage"` and `CATEGORY = "Output"`
- [ ] `SaveImage.INPUT_SLOTS` matches `[SlotSpec("image","IMAGE"), SlotSpec("seed","INT",optional=True), SlotSpec("steps","INT",optional=True)]`
- [ ] `SaveImage.OUTPUT_SLOTS` is `[]`
- [ ] Mock `execute()` emits `ImageReady` event with `width=64`, `height=64`
- [ ] `SaveImage` appears in `NODE_REGISTRY["SaveImage"]` after importing `worker.nodes.image`
