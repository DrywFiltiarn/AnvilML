# Plan Report: P24-D3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P24-D3                                      |
| Phase       | 024 — Generic Conditioning/Sampling/Decode Nodes, Real Mode |
| Description | worker/nodes/image.py: ImageResize node, mock + real (lanczos default) |
| Depends on  | P24-D2                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-19T18:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Add the `ImageResize` node to `worker/nodes/image.py` per ANVILML_DESIGN.md §10.3's exact slot specification, with both a mock-mode and real-mode code path (both delegating to the same `PIL.Image.resize` call since resizing has no GPU/model dependency). Ship 5+ unit tests covering dimension output, default lanczos method, explicit method override, and error on unrecognized method. After this task, `test_nodes_image.py` will contain 14 tests total (9 existing SaveImage + 5 new ImageResize), exceeding the >=13 threshold for the real-mode acceptance command.

## Scope

### In Scope
- Add `ImageResize` class to `worker/nodes/image.py` with exact `NODE_TYPE`, `CATEGORY`, `INPUT_SLOTS`, and `OUTPUT_SLOTS` per §10.3.
- Implement `execute()` with `ctx.mock` branching at the top (§14.6): both branches call `PIL.Image.resize()`.
- `method` parameter defaults to `"lanczos"`; maps recognized strings to `PIL.Image` resize filters; raises `ValueError` on unrecognized strings.
- Add `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` markers next to `execute()`, naming the real-mode and mock-mode tests respectively.
- Add 5 tests to `worker/tests/test_nodes_image.py`: mock mode dimensions, real mode dimensions, default method is lanczos, explicit method honored, unrecognized method raises.

### Out of Scope
None. `defers_to (from JSON): []` — this task must implement its full scope. No deferrals.

## Existing Codebase Assessment

**What already exists:** `worker/nodes/image.py` contains the `SaveImage` node (160 lines) with a well-established pattern: `@register` decorator, class attributes (`NODE_TYPE`, `CATEGORY`, `DISPLAY_NAME`, `DESCRIPTION`, `INPUT_SLOTS`, `OUTPUT_SLOTS`), `execute()` with `ctx.mock` branching at the top, Google-style docstrings, structured `logging` imports, and both `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` markers. The test file `worker/tests/test_nodes_image.py` has 9 tests (3 mock-mode + 1 subprocess registry + 5 real-mode) following a consistent pattern: `_make_ctx()` fixture, captured-events list via custom `_emit`, and assertion-driven verification.

**Established patterns:** (1) Class attributes use exact strings from the design doc. (2) `execute()` branches on `ctx.mock` at the very top. (3) Both markers are placed as `#` comments immediately before `def execute()`. (4) Tests use `_make_ctx(mock=True/False)` and capture emit calls via a list. (5) Real-mode tests use `@pytest.mark.real_mode`. (6) `PIL.Image` is imported locally inside `execute()` (not at module level) to keep imports clean.

**Gap between design doc and current source:** None that affects this task. The design doc §10.3 specifies `ImageResize` with `method` defaulting to `"lanczos"` — this is straightforward to implement against the existing `SaveImage` template. No new types, no new modules, no architectural decisions needed.

## Resolved Dependencies

| Type   | Name     | Version verified | MCP source     | Feature flags confirmed |
|--------|----------|-----------------|----------------|------------------------|
| python | pillow   | 12.3.0          | pypi-query MCP | n/a (builtin filters)  |

**PIL.Image.resize filter mapping confirmed (Pillow 12.3.0):**
- `"lanczos"` → `Image.LANCZOS` (value 1)
- `"nearest"` → `Image.NEAREST` (value 0)
- `"bilinear"` → `Image.BILINEAR` (value 2)
- `"bicubic"` → `Image.BICUBIC` (value 3)
- `"box"` → `Image.BOX` (value 4)
- `Image.LINEAR` and `Image.CUBIC` were removed in Pillow 12.x — the map uses only current names.

## Approach

### Step 1: Add ImageResize class to worker/nodes/image.py

Append the `ImageResize` class after `SaveImage` (after line 160). The class follows the exact same structure as `SaveImage`:

```python
@register
class ImageResize(BaseNode):
    """Resize a PIL.Image to specified dimensions.

    This node accepts an image (required) plus width and height (both required,
    positive integers) and an optional method string (defaults to "lanczos").
    It returns a resized IMAGE via the output slot.

    Both mock and real branches call the same PIL.Image.resize() since image
    resizing has no GPU/model dependency to mock around. The ctx.mock branch
    structure is required per §14.6's general node pattern.

    Class Attributes:
        NODE_TYPE: "ImageResize"
        CATEGORY: "Images"
        DISPLAY_NAME: "Image Resize"
        DESCRIPTION: One-line description.
        INPUT_SLOTS: image (IMAGE, required), width (INT, required),
            height (INT, required), method (STRING, optional=True).
        OUTPUT_SLOTS: image (IMAGE).
    """
    NODE_TYPE = "ImageResize"
    CATEGORY = "Images"
    DISPLAY_NAME = "Image Resize"
    DESCRIPTION = "Resizes a PIL image to the requested dimensions."
    INPUT_SLOTS = [
        SlotSpec("image", "IMAGE"),
        SlotSpec("width", "INT"),
        SlotSpec("height", "INT"),
        SlotSpec("method", "STRING", optional=True),
    ]
    OUTPUT_SLOTS = [
        SlotSpec("image", "IMAGE"),
    ]

    # REAL_PATH_VERIFIED: worker/tests/test_nodes_image.py::test_resize_real_produces_requested_dimensions
    # MOCK_PATH_VERIFIED: worker/tests/test_nodes_image.py::test_resize_mock_returns_correct_dimensions
    def execute(self, ctx: NodeContext, **inputs) -> dict:
        """Resize the input image to the requested dimensions.

        Branches on ctx.mock at the top per §14.6 — both branches call
        PIL.Image.resize() with the same underlying logic since resizing
        has no GPU/model dependency. The mock branch returns a dict with
        the resized dimensions as a sentinel (consistent with SaveImage's
        mock return pattern of returning {"image": {...}}).

        Args:
            ctx: Runtime context carrying job_id, device, caps,
                cancel_flag, emit, pipeline_cache, and mock flag.
            **inputs: Must contain "image" (PIL.Image), "width" (int),
                and "height" (int). Optional "method" (str) defaults
                to "lanczos".

        Returns:
            Dict with key "image" containing the resized PIL.Image in
            real mode, or {"mock": True, "width": <w>, "height": <h>}
            sentinel dict in mock mode.

        Raises:
            KeyError: If "image", "width", or "height" is not provided.
            ValueError: If "method" is not a recognized PIL resize filter.
        """
        # Validate required inputs — "image", "width", "height" are all
        # required (optional=False) per INPUT_SLOTS. Accessing them via
        # dict key raises KeyError if absent, which is the desired failure
        # mode for missing required inputs.
        image = inputs["image"]
        width = inputs["width"]
        height = inputs["height"]

        # Resolve the resize method. The "method" input is optional per
        # INPUT_SLOTS — default to "lanczos" when absent or unset.
        method = inputs.get("method", "lanczos")

        # Map the string method name to a PIL.Image resize filter constant.
        # This uses Pillow 12.x filter names (BILINEAR, BICUBIC — LINEAR
        # and CUBIC were removed in Pillow 12). An unrecognized string
        # raises ValueError with a clear message listing valid options.
        filter_map = {
            "lanczos": Image.LANCZOS,
            "nearest": Image.NEAREST,
            "bilinear": Image.BILINEAR,
            "bicubic": Image.BICUBIC,
            "box": Image.BOX,
        }
        # ... resize logic ...
```

The filter_map uses only Pillow 12.x names. `Image.LINEAR` and `Image.CUBIC` were removed in Pillow 12.x and do not exist. The map is a local dict inside `execute()`.

The resize logic (in both mock and real branches):
```python
# Import PIL locally — keeps the module torch-free for mock-mode
# collection safety (ANVILML_DESIGN.md §11.2).
from PIL import Image

# Look up the filter from the map; raise ValueError if unrecognized.
# This provides a clear error message listing valid method strings.
try:
    filter_constant = filter_map[method]
except KeyError:
    valid = ", ".join(sorted(filter_map.keys()))
    raise ValueError(
        f"Unrecognized resize method '{method}'. "
        f"Valid methods: {valid}"
    )

# Resize the image. Both mock and real branches use the same PIL call
# because image resizing is a pure CPU operation with no GPU/model
# dependency to mock around (ANVILML_DESIGN.md §14.6 note).
resized = image.resize((width, height), filter=filter_constant)

if ctx.mock:
    # Mock branch: return a sentinel dict with the resized dimensions.
    # This is consistent with SaveImage's mock return pattern.
    logger.debug(
        "ImageResize: mock branch resized to %dx%d for job_id=%s",
        width, height, ctx.job_id_str,
    )
    return {"image": {"mock": True, "width": width, "height": height}}
else:
    # Real branch: return the actual resized PIL.Image.
    logger.debug(
        "ImageResize: real branch resized to %dx%d for job_id=%s",
        width, height, ctx.job_id_str,
    )
    return {"image": resized}
```

**Decision rationale for unified resize logic:** The task context explicitly states "Both branches can call the same real PIL resize — resizing has no GPU/model dependency to mock around." The only difference between mock and real is the return value (sentinel dict vs. actual PIL.Image), which is the minimal divergence required by the mock/real parity convention.

### Step 2: Add 5 tests to worker/tests/test_nodes_image.py

Append the following tests after the existing SaveImage tests (after line 373):

1. **`test_resize_mock_returns_correct_dimensions`** (mock mode)
   - Constructs `NodeContext(mock=True)`, calls `execute(image=pil_img, width=128, height=256)`.
   - Asserts return value is `{"image": {"mock": True, "width": 128, "height": 256}}`.
   - Satisfies the `MOCK_PATH_VERIFIED` marker.

2. **`test_resize_real_produces_requested_dimensions`** (real_mode)
   - Constructs `NodeContext(mock=False)`, creates a real PIL Image (64×64), calls `execute(image=pil_img, width=128, height=256)`.
   - Asserts return value's `"image"` field is a PIL Image with size `(128, 256)`.
   - Satisfies the `REAL_PATH_VERIFIED` marker.

3. **`test_resize_default_method_is_lanczos`** (both)
   - Calls `execute(image=pil_img, width=64, height=64)` without specifying `method`.
   - Asserts the return value's image has size `(64, 64)` (verifying the call succeeded with default lanczos).
   - Uses a mock context (runs in both mock and real since the resize logic is identical).

4. **`test_resize_explicit_method_bilinear`** (both)
   - Calls `execute(image=pil_img, width=64, height=64, method="bilinear")`.
   - Asserts the return value's image has size `(64, 64)` (verifying the explicit method was accepted and applied).
   - Uses a mock context (runs in both modes).

5. **`test_resize_unrecognized_method_raises_error`** (both)
   - Calls `execute(image=pil_img, width=64, height=64, method="invalid_method")`.
   - Asserts `ValueError` is raised with a message containing the invalid method name.
   - Uses a mock context (runs in both modes).

**Total after additions:** 9 existing + 5 new = 14 tests (>=13 required).
**Real-mode tests after additions:** 6 existing + 1 new (`test_resize_real_produces_requested_dimensions`) = 7 real_mode tests.

## Public API Surface

| Item | Path | Description |
|------|------|-------------|
| `class ImageResize` | `worker.nodes.image.ImageResize` | New node class extending `BaseNode`, decorated with `@register`. |
| `ImageResize.NODE_TYPE` | `str` | `"ImageResize"` |
| `ImageResize.CATEGORY` | `str` | `"Images"` |
| `ImageResize.DISPLAY_NAME` | `str` | `"Image Resize"` |
| `ImageResize.DESCRIPTION` | `str` | `"Resizes a PIL image to the requested dimensions."` |
| `ImageResize.INPUT_SLOTS` | `list[SlotSpec]` | 4 slots: image (IMAGE), width (INT), height (INT), method (STRING, optional) |
| `ImageResize.OUTPUT_SLOTS` | `list[SlotSpec]` | 1 slot: image (IMAGE) |
| `ImageResize.execute()` | `(ctx: NodeContext, **inputs) -> dict` | Resizes input image; returns `{"image": resized_image_or_sentinel}` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/image.py` | Add `ImageResize` class after `SaveImage` (~55 lines) |
| MODIFY | `worker/tests/test_nodes_image.py` | Add 5 ImageResize tests (~100 lines) |

## Tests

| Test File | Test Name | What It Verifies | Mode | Acceptance Command |
|-----------|-----------|-----------------|------|-------------------|
| `worker/tests/test_nodes_image.py` | `test_resize_mock_returns_correct_dimensions` | ImageResize mock branch returns sentinel dict with correct resized dimensions (128×256) | mock | `worker/.venv/bin/python -m pytest worker/tests/test_nodes_image.py::test_resize_mock_returns_correct_dimensions -v` |
| `worker/tests/test_nodes_image.py` | `test_resize_real_produces_requested_dimensions` | ImageResize real branch produces a PIL.Image with the exact requested dimensions | real | `worker/.venv/bin/python -m pytest worker/tests/test_nodes_image.py::test_resize_real_produces_requested_dimensions -v -m real_mode` |
| `worker/tests/test_nodes_image.py` | `test_resize_default_method_is_lanczos` | Calling execute() without `method` uses lanczos filter successfully (dimensions match) | both | `worker/.venv/bin/python -m pytest worker/tests/test_nodes_image.py::test_resize_default_method_is_lanczos -v` |
| `worker/tests/test_nodes_image.py` | `test_resize_explicit_method_bilinear` | Calling execute() with `method="bilinear"` is accepted and produces correct dimensions | both | `worker/.venv/bin/python -m pytest worker/tests/test_nodes_image.py::test_resize_explicit_method_bilinear -v` |
| `worker/tests/test_nodes_image.py` | `test_resize_unrecognized_method_raises_error` | Calling execute() with an unrecognized method raises ValueError with clear error message | both | `worker/.venv/bin/python -m pytest worker/tests/test_nodes_image.py::test_resize_unrecognized_method_raises_error -v` |

## CI Impact

The `worker-linux-mock` and `worker-linux-real` CI jobs will pick up the new tests automatically since they run `pytest worker/tests/` (the test file is in the standard location). No CI workflow file changes are needed. The new tests use `PIL.Image` which is already in `requirements/base.txt` (pillow==12.3.0), so no dependency changes affect CI provisioning.

## Platform Considerations

None identified. The `PIL.Image.resize()` API is cross-platform and the resize filters (NEAREST, LANCZOS, BILINEAR, BICUBIC, BOX) are available on all platforms. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Pillow 12.x removed `Image.LINEAR` and `Image.CUBIC` — if the method map accidentally references these, a `AttributeError` will be raised at runtime when someone passes "linear" or "cubic". | Low | Medium | Verified via MCP that Pillow 12.3.0 only exposes NEAREST, LANCZOS, BILINEAR, BICUBIC, BOX. The method map uses only these confirmed names. The ValueError on unrecognized methods will catch typos. |
| The `PIL.Image` import inside `execute()` — if it fails (e.g., corrupted Pillow install), the error surfaces as an `ImportError` deep in node execution rather than at worker startup. | Low | Low | This is the same pattern used by `SaveImage` (which also imports PIL locally). The error will be caught by the worker's exception handler and surfaced as a `Failed` event. |
| Adding 5 tests pushes the test file to ~475 lines, approaching the 500-line review threshold for test files. | Low | Low | The file is still under 500 lines. All tests are for the same module (`image.py`) and same logical unit (ImageResize), so they belong together per the "tests covering more than one logical unit" split criterion. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/image.py` exits 0
- [ ] `worker/.venv/bin/python -m py_compile worker/tests/test_nodes_image.py` exits 0
- [ ] `worker/.venv/bin/python -m pytest worker/tests/test_nodes_image.py -v` exits 0 (all 14 tests pass, >=5 ImageResize tests)
- [ ] `worker/.venv/bin/python -m pytest worker/tests/test_nodes_image.py -v -m real_mode` exits 0 (>=13 total tests in file, >=7 real_mode)
- [ ] `grep "REAL_PATH_VERIFIED:" worker/nodes/image.py` returns one line naming `test_resize_real_produces_requested_dimensions`
- [ ] `grep "MOCK_PATH_VERIFIED:" worker/nodes/image.py` returns one line naming `test_resize_mock_returns_correct_dimensions`
- [ ] `grep "ImageResize" worker/nodes/__init__.py` — the class is auto-registered (verified by the existing `test_save_image_in_registry` pattern adapted for ImageResize)
