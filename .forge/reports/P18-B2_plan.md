# Plan Report: P18-B2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-B2                                      |
| Phase       | 018 — ZiT Generic Nodes                     |
| Description | worker/nodes/sampler.py: EmptyLatent and Sampler nodes |
| Depends on  | P18-A1, P18-A2, P18-A3, P18-B1              |
| Project     | anvilml                                     |
| Planned at  | 2026-06-21T13:58:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `worker/nodes/sampler.py` implementing the `EmptyLatent` and `Sampler` nodes as
defined in `ANVILML_DESIGN.md §10.3`. `EmptyLatent` creates a blank noise latent tensor
at the requested resolution (mock: `MockLatent` sentinel). `Sampler` runs the denoising
loop (mock: emit 3 `Progress` events via `ctx.emit`, return `MockLatent` + resolved seed).
Both nodes register themselves in `NODE_REGISTRY` via the `@register` decorator. The
`Sampler` node sets `EMITS_PROGRESS = True` so the executor's progress-emission path
(in `executor.py` lines 215–228) activates during mock-mode graph execution.

## Scope

### In Scope
- Create `worker/nodes/sampler.py` with:
  - `MockLatent` sentinel class (carries `width`, `height`, `batch_size`).
  - `EmptyLatent` node: `INPUT_SLOTS=[width:INT, height:INT, batch_size:INT?]`,
    `OUTPUT_SLOTS=[latent:LATENT]`. Mock returns `{"latent": MockLatent}`.
  - `Sampler` node: `INPUT_SLOTS=[model:MODEL, conditioning:CONDITIONING, latent:LATENT, steps:INT, cfg:FLOAT, seed:INT]`,
    `OUTPUT_SLOTS=[latent:LATENT, seed:INT]`. Mock resolves `seed=-1` to `randint(1, 2**32-1)`,
    emits 3 Progress events, returns `{"latent": MockLatent, "seed": resolved_seed}`.
  - Real-mode stubs: `raise NotImplementedError(...)` with TODO references.
- Create `worker/tests/test_nodes_sampler.py` with ≥5 tests.

### Out of Scope
- Real-mode sampling implementation (deferred to P18-C1 which creates `arch/zit.py`).
- `arch/` module creation (`worker/nodes/arch/__init__.py`, `arch/zit.py`, `arch/flux.py`)
  — these are P18-C1 scope.
- `ImageResize` node — separate task in Phase 018.
- Integration with the executor's graph execution (the executor already supports
  `EMITS_PROGRESS` nodes per `executor.py` lines 215–228).

## Existing Codebase Assessment

The existing codebase has a well-established node registration pattern. `worker/nodes/base.py`
defines `BaseNode` (ABC with `execute()`, metadata attributes, `NodeContext`), `SlotSpec`
(dataclass with `name`, `slot_type`, `optional`), `NodeContext` (job_id, device, cancel_flag,
emit, pipeline_cache), and the `@register` decorator that validates six required attributes
and stores the class in the global `NODE_REGISTRY`.

`worker/nodes/loader.py` (P18-A1..A3) provides the established mock pattern: each node
checks `os.environ.get("ANVILML_WORKER_MOCK") == "1"` at runtime, returns a lightweight
sentinel class (`MockModel`, `MockVae`, `MockClip`) in mock mode, and raises
`NotImplementedError` in real mode with a TODO reference.

`worker/nodes/encoder.py` (P18-B1) follows the same pattern for `ClipTextEncode`.

`worker/executor.py` already has built-in support for progress events: when a node's
class has `EMITS_PROGRESS = True`, the executor emits 3 `Progress` events in mock mode
(lines 215–228). This means the `Sampler` node only needs to set the class attribute —
no manual `ctx.emit` calls for progress are required from the node's `execute()` method.

No `MockLatent` class exists yet; it must be created in this task. The `arch/` directory
does not exist yet (P18-C1 creates it). The test style uses `importlib.reload()` to
re-register nodes against a cleared `NODE_REGISTRY`, a `registry_clean` autouse fixture,
and a `mock_context` fixture with a captured `emit` list.

## Resolved Dependencies

None. This task introduces no new external dependencies. All types (`MockLatent`,
`EmptyLatent`, `Sampler`) are new internal classes. The only imports are from the
existing `worker.nodes.base` module and Python standard library (`random`, `os`).

| Type   | Name          | Version verified | MCP source | Feature flags confirmed |
|--------|---------------|-----------------|------------|------------------------|
| stdlib | random        | (stdlib)        | n/a        | n/a                    |
| stdlib | os            | (stdlib)        | n/a        | n/a                    |

## Approach

1. **Create `worker/nodes/sampler.py`.** This is a new file following the established
   pattern from `loader.py` and `encoder.py`. The file docstring must describe the module
   purpose, the mock-mode guard pattern, and include a `.. versionadded:: 0.1.0` tag.
   The `__all__` exports `EmptyLatent`, `Sampler`, and `MockLatent`.

2. **Define `MockLatent` sentinel class.** A lightweight class carrying `width`, `height`,
   and `batch_size` attributes. Docstring: Google style, explains it stands in for a real
   latent tensor during testing. Constructor stores the three dimensions. This mirrors
   `MockModel` / `MockVae` / `MockClip` in `loader.py`.

3. **Implement `EmptyLatent` node.** Decorate with `@register`. Set metadata attributes:
   `NODE_TYPE = "EmptyLatent"`, `CATEGORY = "Latents"`, `DISPLAY_NAME = "Empty Latent"`,
   `DESCRIPTION = "Create a blank noise latent tensor at the requested resolution"`.
   `INPUT_SLOTS = [SlotSpec("width", "INT"), SlotSpec("height", "INT"),
   SlotSpec("batch_size", "INT", optional=True)]`.
   `OUTPUT_SLOTS = [SlotSpec("latent", "LATENT")]`.
   `execute()` reads `width`, `height`, optional `batch_size` (default 1), checks mock
   mode, and returns `{"latent": MockLatent(width, height, batch_size)}` in mock mode.
   Real mode raises `NotImplementedError("Real EmptyLatent path not yet implemented …")`.
   All decision points get inline `#` comments.

4. **Implement `Sampler` node.** Decorate with `@register`. Set metadata attributes:
   `NODE_TYPE = "Sampler"`, `CATEGORY = "Sampling"`, `DISPLAY_NAME = "Sampler"`,
   `DESCRIPTION = "Run the denoising sampling loop"`.
   `INPUT_SLOTS = [SlotSpec("model", "MODEL"), SlotSpec("conditioning", "CONDITIONING"),
   SlotSpec("latent", "LATENT"), SlotSpec("steps", "INT"), SlotSpec("cfg", "FLOAT"),
   SlotSpec("seed", "INT")]`.
   `OUTPUT_SLOTS = [SlotSpec("latent", "LATENT"), SlotSpec("seed", "INT")]`.
   Set `EMITS_PROGRESS = True` so the executor's progress-emission path activates.
   `execute()` reads all inputs, checks mock mode:
   - If `seed == -1`, resolve to `random.randint(1, 2**32 - 1)` (the standard ComfyUI
     convention: -1 means "pick a random seed").
   - Return `{"latent": MockLatent(latent.width, latent.height, latent.batch_size),
     "seed": resolved_seed}`.
   Real mode raises `NotImplementedError` with a TODO referencing P18-C1 for the
   `arch.sample()` dispatch path.

5. **Create `worker/tests/test_nodes_sampler.py`.** Follow the established test pattern
   from `test_nodes_loader.py` and `test_nodes_encoder.py`:
   - `registry_clean` autouse fixture (clears `NODE_REGISTRY`).
   - `mock_context` fixture (builds `NodeContext` with captured emit list).
   - Tests:
     a. `test_emptylatent_registered_in_registry` — reload module, assert "EmptyLatent" in
        `NODE_REGISTRY`.
     b. `test_emptylatent_execute_returns_mock_latent` — instantiate, call `execute()`,
        assert result contains `MockLatent` with correct width/height/batch_size.
     c. `test_emptylatent_default_batch_size` — call without `batch_size`, assert defaults
        to 1.
     d. `test_sampler_registered_in_registry` — reload module, assert "Sampler" in
        `NODE_REGISTRY`.
     e. `test_sampler_execute_returns_mock_latent_and_seed` — instantiate with mock inputs,
        call `execute(seed=42)`, assert result contains `MockLatent` and `"seed": 42`.
     f. `test_sampler_seed_negative_one_resolves_to_random` — call `execute(seed=-1)`,
        assert returned seed is in range `[1, 2**32 - 1]` (not -1).
     g. `test_sampler_emits_progress` — set `EMITS_PROGRESS = True`, run through executor
        or directly verify `ctx.emit` was called (the executor handles this automatically
        when `EMITS_PROGRESS` is True; test by calling `execute()` and asserting the
        `mock_context` emit list has entries — actually, the executor emits progress,
        not the node itself. The node just returns its outputs. So this test should
        verify the attribute exists: `assert Sampler.EMITS_PROGRESS is True`).
     h. `test_sampler_metadata_attributes` — verify all six metadata attributes.
     i. `test_emptylatent_metadata_attributes` — verify all six metadata attributes.

   Total: 9 tests (≥ 5 required).

6. **Run Python syntax check.** Before running pytest, execute
   `worker/.venv/bin/python -m py_compile worker/nodes/sampler.py` to confirm no
   syntax errors, per ENVIRONMENT.md §7 (Step 7).

7. **Run tests.** Execute
   `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py -v`
   and confirm exit 0 with all tests passing.

## Public API Surface

All items are in `worker/nodes/sampler.py` (Python package — no `pub` in Python, but
these are the public interface of the module):

| Item | Type | Module Path | Description |
|------|------|-------------|-------------|
| `MockLatent` | class | `worker.nodes.sampler` | Sentinel latent object carrying `width`, `height`, `batch_size` |
| `MockLatent.__init__(width: int, height: int, batch_size: int = 1)` | method | `worker.nodes.sampler.MockLatent` | Construct mock latent |
| `EmptyLatent` | class (node) | `worker.nodes.sampler.EmptyLatent` | Node: width×height latent creation, registered as `"EmptyLatent"` |
| `EmptyLatent.execute(**inputs: Any) -> dict[str, Any]` | method | `worker.nodes.sampler.EmptyLatent` | Return `{"latent": MockLatent(...)}` in mock mode |
| `Sampler` | class (node) | `worker.nodes.sampler.Sampler` | Node: denoising loop, registered as `"Sampler"`, `EMITS_PROGRESS = True` |
| `Sampler.execute(**inputs: Any) -> dict[str, Any]` | method | `worker.nodes.sampler.Sampler` | Return `{"latent": MockLatent, "seed": int}` in mock mode |

Registration keys in `NODE_REGISTRY`:
- `"EmptyLatent"` → `EmptyLatent` class
- `"Sampler"` → `Sampler` class

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/sampler.py` | EmptyLatent + Sampler nodes with MockLatent sentinel |
| CREATE | `worker/tests/test_nodes_sampler.py` | ≥5 tests for both nodes and MockLatent |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_nodes_sampler.py` | `test_emptylatent_registered_in_registry` | `EmptyLatent` is in `NODE_REGISTRY` after import | `NODE_REGISTRY` cleared by `registry_clean` fixture | None | `"EmptyLatent" in NODE_REGISTRY` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py::test_emptylatent_registered_in_registry -v` exits 0 |
| `worker/tests/test_nodes_sampler.py` | `test_emptylatent_execute_returns_mock_latent` | `execute()` returns `MockLatent` with correct dimensions | `ANVILML_WORKER_MOCK=1` set by conftest | `width=512, height=512, batch_size=4` | `result["latent"]` is `MockLatent(512, 512, 4)` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py::test_emptylatent_execute_returns_mock_latent -v` exits 0 |
| `worker/tests/test_nodes_sampler.py` | `test_emptylatent_default_batch_size` | `batch_size` defaults to 1 when omitted | `ANVILML_WORKER_MOCK=1` set by conftest | `width=512, height=512` (no batch_size) | `result["latent"].batch_size == 1` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py::test_emptylatent_default_batch_size -v` exits 0 |
| `worker/tests/test_nodes_sampler.py` | `test_sampler_registered_in_registry` | `Sampler` is in `NODE_REGISTRY` after import | `NODE_REGISTRY` cleared by `registry_clean` fixture | None | `"Sampler" in NODE_REGISTRY` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_registered_in_registry -v` exits 0 |
| `worker/tests/test_nodes_sampler.py` | `test_sampler_execute_returns_mock_latent_and_seed` | `execute()` returns `MockLatent` + correct seed passthrough | `ANVILML_WORKER_MOCK=1` set by conftest | `model=MockModel(), conditioning=MockConditioning(), latent=MockLatent(512,512), steps=4, cfg=7.0, seed=42` | `result["seed"] == 42`, `result["latent"]` is `MockLatent` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_execute_returns_mock_latent_and_seed -v` exits 0 |
| `worker/tests/test_nodes_sampler.py` | `test_sampler_seed_negative_one_resolves_to_random` | `seed=-1` resolves to a random integer in `[1, 2**32-1]` | `ANVILML_WORKER_MOCK=1` set by conftest | `seed=-1` | `1 <= result["seed"] <= 4294967295` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_seed_negative_one_resolves_to_random -v` exits 0 |
| `worker/tests/test_nodes_sampler.py` | `test_sampler_emits_progress_flag` | `Sampler.EMITS_PROGRESS` is `True` so executor emits Progress events | None | None | `Sampler.EMITS_PROGRESS is True` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_emits_progress_flag -v` exits 0 |
| `worker/tests/test_nodes_sampler.py` | `test_sampler_metadata_attributes` | All six metadata attributes on `Sampler` are correct | None | None | `NODE_TYPE=="Sampler"`, `CATEGORY=="Sampling"`, etc. | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py::test_sampler_metadata_attributes -v` exits 0 |
| `worker/tests/test_nodes_sampler.py` | `test_emptylatent_metadata_attributes` | All six metadata attributes on `EmptyLatent` are correct | None | None | `NODE_TYPE=="EmptyLatent"`, `CATEGORY=="Latents"`, etc. | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py::test_emptylatent_metadata_attributes -v` exits 0 |

## CI Impact

No CI changes required. The new test file `worker/tests/test_nodes_sampler.py` is picked
up automatically by the existing CI `worker-linux` and `worker-windows` jobs which run
`pytest worker/tests/ -v` (the glob pattern `worker/tests/` already captures all test files
in that directory). No new CI gates, jobs, or configuration changes are needed.

## Platform Considerations

None identified. The `sampler.py` module uses only Python standard library (`random`, `os`)
and existing worker infrastructure types. No `#[cfg(...)]` guards are relevant (this is
Python code, not Rust). Path handling, line endings, and environment variable access all
follow the established cross-platform patterns already verified in `loader.py` and
`encoder.py`. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `random.randint(1, 2**32-1)` range differs from ComfyUI convention (which uses `random.randrange(0, 2**32)` yielding `[0, 2**32-1]`). If downstream code expects seed 0 to be valid from the -1 resolution, this would produce an off-by-one gap. | Low | Medium | Use `random.randrange(0, 2**32)` to match ComfyUI's exact range `[0, 2**32-1]`. This is the safer choice since seed 0 is a common explicit seed value. |
| The `Sampler` node's `execute()` method accesses `latent.width`, `latent.height`, `latent.batch_size` attributes. If a test passes a `None` latent (missing input), this will raise `AttributeError`. | Low | Low | Use `inputs.get("latent")` which may be `None`; in mock mode, guard with `if latent is not None:` before accessing attributes, defaulting to `MockLatent(512, 512)` for missing latent. |
| `EMITS_PROGRESS = True` on the `Sampler` class triggers the executor's progress-emission path (lines 215–228 of `executor.py`). If the executor is not yet integrated into the test flow, the progress events won't be observable in unit tests. | Low | Low | Test the attribute directly (`assert Sampler.EMITS_PROGRESS is True`) rather than trying to observe executor-side progress emissions in a unit test. The executor's own progress-emission logic is tested separately in `test_executor.py`. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/nodes/sampler.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_sampler.py -v` exits 0 with ≥ 5 tests passing
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v` exits 0 (full test suite, no regression)
