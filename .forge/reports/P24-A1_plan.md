# Plan Report: P24-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P24-A1                                      |
| Phase       | 24 — Generic Conditioning/Sampling/Decode Nodes, Real Mode |
| Description | worker/nodes/encoder.py: ClipTextEncode node, mock branch only |
| Depends on  | P22-D1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-17T18:25:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `worker/nodes/encoder.py` containing the `ClipTextEncode` node class with a fully
working mock branch (per `ANVILML_DESIGN.md §14.6`) and a bare `raise NotImplementedError`
placeholder for the real branch, registered via `@register` so it appears in
`NODE_REGISTRY`. This gives the conditioning pipeline its first text-encoding node,
enabling downstream graph wiring (`ClipTextEncode → CONDITIONING → Sampler`) in subsequent
tasks. The acceptance criterion is `python -m pytest worker/tests/test_nodes_encoder.py -v`
exiting 0 with ≥3 tests.

## Scope

### In Scope
- Create `worker/nodes/encoder.py` with the `ClipTextEncode` class.
- Define `NODE_TYPE="ClipTextEncode"`, `CATEGORY="Conditioning"`, `DISPLAY_NAME`,
  `DESCRIPTION`, `INPUT_SLOTS`, `OUTPUT_SLOTS` per `ANVILML_DESIGN.md §10.3`.
- Implement `execute(self, ctx: NodeContext, **inputs) -> dict` with:
  - **Mock branch** (`ctx.mock` is True): returns `{"conditioning": {"mock": True, "positive_text": inputs["positive_text"]}}`.
  - **Real branch** (`ctx.mock` is False): raises `NotImplementedError` (placeholder for P24-A2).
- Add `@register` decorator to auto-register in `NODE_REGISTRY`.
- Add dual-mode parity markers (`REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED`) next to
  `execute()` — the mock marker names a test added in this task; the real marker names
  a test that will be added in P24-A2 (the deferring task).
- Create `worker/tests/test_nodes_encoder.py` with ≥3 tests.
- Update `docs/TESTS.md` with entries for the new tests.

### Out of Scope
- Real branch implementation (tokenize + encode): deferred to P24-A2.
  `defers_to: ["P24-A2"]` — P24-A2's context states "Complete ClipTextEncode's real
  branch (worker/nodes/encoder.py), the scope P24-A1 deferred: calls inputs["clip"]'s
  tokenizer ... then the encoder's forward pass." This genuinely covers the deferred
  scope.
- Architecture-specific dispatch logic: `ClipTextEncode` is architecture-agnostic per
  §10.3; the `clip` input already carries its own tokenizer and model from `LoadClip`.

## Existing Codebase Assessment

**What already exists:** The node system is fully established in `worker/nodes/base.py`
with `BaseNode` (ABC), `@register` decorator, `SlotSpec` dataclass, `NodeContext` class,
and `NODE_REGISTRY` dict. Auto-import of node modules happens in `worker/nodes/__init__.py`
via `pkgutil.iter_modules()`, skipping `__init__`, `base`, and packages (so `arch/` is
not recursed). Existing nodes (`LoadModel`, `LoadVae`, `LoadClip` in `loader.py`;
`Sampler` in `sampler.py`) all follow the same pattern: class attributes, `@register`,
`execute()` branching on `ctx.mock`, dual-mode parity markers, and logging.

**Established patterns:**
- Class attributes: `NODE_TYPE`, `CATEGORY`, `DISPLAY_NAME`, `DESCRIPTION`,
  `INPUT_SLOTS`, `OUTPUT_SLOTS` — all defined as class-level attributes.
- `execute()` signature: `def execute(self, ctx: NodeContext, **inputs) -> dict:`
- Mock branch: first `if ctx.mock:` branch returning a sentinel dict with
  `{"mock": True, ...}` carrying the relevant input value(s).
- Real branch: `else:` branch with `raise NotImplementedError("...")` for stubbed nodes.
- Parity markers: two `# REAL_PATH_VERIFIED:` / `# MOCK_PATH_VERIFIED:` comment lines
  immediately above the `execute()` method.
- Logging: `logger = logging.getLogger(__name__)` at module level; DEBUG-level logs
  inside execute.
- Tests: `_make_ctx()` helper in each test file; subprocess isolation for registry tests;
  `@pytest.mark.real_mode` decorator for real-mode tests.

**Gap between design doc and source:** None relevant to this task. The design doc's
§10.3 node table and §14.6 mock behavior are fully consistent with the existing node
implementations in `loader.py` and `sampler.py`.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| python | pytest  | (from existing venv) | pypi-query MCP | n/a |

No new external dependencies are introduced. This task only uses Python stdlib
(`threading`, `subprocess`, `sys`, `uuid`) and existing project packages
(`worker.nodes.base`, `pytest`). The `logging` module is stdlib.

## Approach

1. **Create `worker/nodes/encoder.py`.**
   - Import `BaseNode`, `NodeContext`, `SlotSpec`, `register` from `worker.nodes.base`.
   - Import `logging` and create `logger = logging.getLogger(__name__)`.
   - Define `class ClipTextEncode(BaseNode):` with:
     - `NODE_TYPE = "ClipTextEncode"`
     - `CATEGORY = "Conditioning"`
     - `DISPLAY_NAME = "Clip Text Encode"`
     - `DESCRIPTION = "Encodes a text prompt using a loaded CLIP-compatible encoder."`
     - `INPUT_SLOTS = [SlotSpec("clip", "CLIP"), SlotSpec("positive_text", "STRING"), SlotSpec("negative_text", "STRING", optional=True)]`
     - `OUTPUT_SLOTS = [SlotSpec("conditioning", "CONDITIONING")]`
   - Add parity markers above `execute()`:
     ```python
     # REAL_PATH_VERIFIED: worker/tests/test_nodes_encoder.py::test_clip_text_encode_real_raises_placeholder
     # MOCK_PATH_VERIFIED: worker/tests/test_nodes_encoder.py::test_clip_text_encode_mock_returns_sentinel
     ```
   - Implement `execute(self, ctx: NodeContext, **inputs) -> dict`:
     - Mock branch (`if ctx.mock:`): return
       `{"conditioning": {"mock": True, "positive_text": inputs["positive_text"]}}`.
       Add a DEBUG log: `logger.debug("ClipTextEncode: mock mode, positive_text=%s", inputs["positive_text"])`.
     - Real branch (`else:`): raise `NotImplementedError("real branch in P24-A2")`.
       (This is the placeholder — P24-A2 will replace it with actual tokenization and encoding.)

2. **Create `worker/tests/test_nodes_encoder.py`.**
   - Create `_make_ctx(mock=True)` helper (same pattern as `test_nodes_loader.py`).
   - Test 1: `test_clip_text_encode_class_attributes` — verify all six required class
     attributes exist with correct values (NODE_TYPE, CATEGORY, DISPLAY_NAME, DESCRIPTION,
     INPUT_SLOTS count and types, OUTPUT_SLOTS count and types).
   - Test 2: `test_clip_text_encode_mock_returns_sentinel` — construct a NodeContext with
     `mock=True`, call `execute()` with `clip={}`, `positive_text="a red fox"`, verify
     return is `{"conditioning": {"mock": True, "positive_text": "a red fox"}}`. This
     exercises the mock code path and satisfies `MOCK_PATH_VERIFIED`.
   - Test 3: `test_clip_text_encode_mock_without_negative_text` — call `execute()` with
     only `clip={}` and `positive_text="hello"` (omit `negative_text`), verify no error
     is raised and the sentinel is returned correctly. This tests the optional input
     slot handling.
   - Test 4: `test_clip_text_encode_in_registry` — subprocess isolation test that imports
     `worker.nodes.encoder` and verifies `NODE_REGISTRY["ClipTextEncode"]` exists.

3. **Update `docs/TESTS.md`.** Add entries for the four new tests with their Mode field
   (mock/real/both) and context.

## Public API Surface

| Item | Module Path | Description |
|------|-------------|-------------|
| `class ClipTextEncode(BaseNode)` | `worker.nodes.encoder` | New node class for text encoding. |
| `ClipTextEncode.NODE_TYPE` | `worker.nodes.encoder` | `"ClipTextEncode"` |
| `ClipTextEncode.CATEGORY` | `worker.nodes.encoder` | `"Conditioning"` |
| `ClipTextEncode.INPUT_SLOTS` | `worker.nodes.encoder` | 3 SlotSpecs: clip(CLIP), positive_text(STRING), negative_text(STRING, optional) |
| `ClipTextEncode.OUTPUT_SLOTS` | `worker.nodes.encoder` | 1 SlotSpec: conditioning(CONDITIONING) |
| `ClipTextEncode.execute()` | `worker.nodes.encoder` | `def execute(self, ctx: NodeContext, **inputs) -> dict` — mock returns sentinel, real raises |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/encoder.py` | New file: `ClipTextEncode` node class with mock branch and real placeholder |
| CREATE | `worker/tests/test_nodes_encoder.py` | New test file: ≥3 tests for ClipTextEncode |
| MODIFY | `docs/TESTS.md` | Add entries for new encoder tests |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_nodes_encoder.py` | `test_clip_text_encode_class_attributes` | All six class attributes exist with correct NODE_TYPE, CATEGORY, slot counts/types | None (import test module) | N/A | All assertions pass | `python -m pytest worker/tests/test_nodes_encoder.py::test_clip_text_encode_class_attributes -v` exits 0 |
| `worker/tests/test_nodes_encoder.py` | `test_clip_text_encode_mock_returns_sentinel` (mock) | Mock-mode execute() returns the sentinel dict with propagated positive_text | NodeContext with mock=True | `clip={}`, `positive_text="a red fox"` | `{"conditioning": {"mock": True, "positive_text": "a red fox"}}` | `python -m pytest worker/tests/test_nodes_encoder.py::test_clip_text_encode_mock_returns_sentinel -v` exits 0 |
| `worker/tests/test_nodes_encoder.py` | `test_clip_text_encode_mock_without_negative_text` (mock) | Omitting optional negative_text input doesn't error | NodeContext with mock=True | `clip={}`, `positive_text="hello"` (no negative_text) | `{"conditioning": {"mock": True, "positive_text": "hello"}}` | `python -m pytest worker/tests/test_nodes_encoder.py::test_clip_text_encode_mock_without_negative_text -v` exits 0 |
| `worker/tests/test_nodes_encoder.py` | `test_clip_text_encode_in_registry` | ClipTextEncode appears in NODE_REGISTRY after importing the module | Subprocess isolation | N/A | `NODE_REGISTRY["ClipTextEncode"]` exists | `python -m pytest worker/tests/test_nodes_encoder.py::test_clip_text_encode_in_registry -v` exits 0 |

## CI Impact

No CI changes required. The new test file follows the existing convention of one test
file per source module under `worker/tests/`. The `worker-linux-mock` and
`worker-windows-mock` CI jobs will automatically pick up the new tests via
`pytest worker/tests/ -v -m "not real_mode"` (all four tests have no `real_mode`
marker, so they run in both mock and real CI jobs). No new file types, gates, or
CI configuration changes are needed.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. This task
only uses Python stdlib (`threading`, `subprocess`, `sys`, `uuid`, `logging`) and
project-internal imports — no platform-specific code paths, no path separators, no
line-ending handling.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `negative_text` optional input handling: if the executor passes `negative_text=None` instead of omitting the key entirely, `inputs.get("negative_text")` vs `inputs["negative_text"]` matters. The mock branch only reads `positive_text`, so this risk does not affect mock-mode tests. | Low | Low | Mock branch accesses `inputs["positive_text"]` (required) and does not touch `negative_text`. The real branch placeholder (`raise`) also doesn't access it. No code change needed. |
| Dual-mode parity markers point to a real-mode test that doesn't exist yet (P24-A2). Gate 4's `grep -L "REAL_PATH_VERIFIED:"` will find this file but the named test won't be collectible until P24-A2. | Medium | Low | This is the expected pattern for deferred real branches. The marker convention (§10.6) requires both markers on every execute() — the plan adds both. Gate 4's test-collectibility check will flag the real marker as uncollectible until P24-A2, which is the correct behavior (it surfaces the incomplete state). |
| Test file naming conflict: `test_nodes_encoder.py` might be mistaken for testing the encoder arch module (`arch/clip/qwen3.py`). | Low | Low | The test file name matches the source module name (`encoder.py`), consistent with the project's convention (`test_ipc.py` for `ipc.py`, `test_executor.py` for `executor.py`). No conflict. |

## Acceptance Criteria

- [ ] `python -m py_compile worker/nodes/encoder.py` exits 0
- [ ] `python -m py_compile worker/tests/test_nodes_encoder.py` exits 0
- [ ] `python -m pytest worker/tests/test_nodes_encoder.py -v` exits 0 with ≥3 tests
- [ ] `grep -rn "REAL_PATH_VERIFIED:" worker/nodes/encoder.py` returns exactly 1 match
- [ ] `grep -rn "MOCK_PATH_VERIFIED:" worker/nodes/encoder.py` returns exactly 1 match
- [ ] `grep -rn "ClipTextEncode" worker/nodes/__pycache__/` or subprocess test confirms `NODE_REGISTRY["ClipTextEncode"]` exists
