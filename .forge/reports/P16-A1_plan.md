# Plan Report: P16-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P16-A1                                        |
| Phase       | 016 — Live Job Events                         |
| Description | anvilml-worker: progress reporting in executor.py + worker_main.py |
| Depends on  | P15-A1 (NodeContext.emit populated), P15-A2 (ImageReady relay) |
| Project     | anvilml                                       |
| Planned at  | 2026-06-20T19:45:00Z                          |
| Attempt     | 1                                             |

## Objective

Extend the Python worker's graph execution engine (`executor.py`) to emit `Progress` IPC events after each node execution when the node declares `EMITS_PROGRESS = True`. In mock mode (`ANVILML_WORKER_MOCK=1`), the executor emits exactly 3 Progress events (step=1,2,3, total_steps=3, preview_b64=None) before the node's own events (e.g. SaveImage's ImageReady). This produces a predictable observable event sequence that integration tests and frontend developers can rely on. The `emit` callable is already wired into `NodeContext` in `worker_main.py` — this task ensures it is actively used by the executor.

## Scope

### In Scope
- Add `EMITS_PROGRESS: bool = False` class attribute to `BaseNode` in `worker/nodes/base.py`
- Modify `run_graph()` in `worker/executor.py` to check `EMITS_PROGRESS` on each executed node and emit Progress events via `ctx.emit`
- In mock mode: emit exactly 3 Progress events (`step=1,2,3`, `total_steps=3`, `preview_b64=None`) for any node with `EMITS_PROGRESS = True`
- In non-mock mode: emit exactly 1 Progress event (`step=N`, `total_steps=N`, `preview_b64=None`) for any node with `EMITS_PROGRESS = True`
- Add test in `worker/tests/test_executor.py` that verifies Progress events are emitted in order before ImageReady for a mock step-based node
- Verify `worker_main.py` already passes `emit=send_event` into `NodeContext` (it does at line 226)

### Out of Scope
- Adding the `EMITS_PROGRESS` attribute to any concrete node class (Sampler node not yet implemented in this phase)
- Relaying Progress events from Rust scheduler to WebSocket (that is P16-A2)
- Persisting Progress events to SQLite (explicitly excluded by Phase 016 constraints)
- Modifying the `WorkerEvent::Progress` Rust struct — it already exists

## Existing Codebase Assessment

**What already exists:**
- `NodeContext` in `worker/nodes/base.py` already carries an `emit: Callable[..., None]` field (added in Phase 011), and `worker_main.py` already populates it with `send_event` at line 226. No changes needed to `NodeContext` or `worker_main.py`.
- `WorkerEvent::Progress` exists in `crates/anvilml-ipc/src/messages.rs` (line 146–156) with fields `job_id: Uuid`, `step: u32`, `total_steps: u32`, `preview_b64: Option<String>`.
- `WsEvent::JobProgress` exists in `crates/anvilml-core/src/types/events.rs` (line 56–65) with the same fields.
- `SaveImage` in `worker/nodes/image.py` already demonstrates the pattern: it calls `self.ctx.emit({"_type": "ImageReady", ...})` after computation. This is the exact pattern to follow for Progress events.
- `run_graph()` in `worker/executor.py` (line 125–215) performs topological sort, instantiates nodes, resolves inputs, calls `node.execute()`, and stores outputs — but currently emits no Progress events.

**Established patterns:**
- Event emission uses `ctx.emit({ "_type": "...", ... })` with a dict that has a `_type` discriminator key matching the Rust enum variant name.
- Tests use `NODE_REGISTRY.clear()` in an `autouse` fixture for isolation.
- Test nodes are created via `_make_node_class()` helper that wraps a function in a `@register`-decorated class.
- The `mock_context` fixture provides a `NodeContext` with a captured `emit` callable that stores events in a list.
- `ANVILML_WORKER_MOCK` is set to `"1"` by the `conftest.py` autouse fixture, so all tests run in mock mode.

**Gap between design doc and current source:**
- No `EMITS_PROGRESS` attribute exists on `BaseNode` or any node class. This must be added.
- No Progress event emission exists in `run_graph()`. This is the primary addition.
- The Sampler node does not exist yet (not in the nodes directory), so no existing node declares `EMITS_PROGRESS = True`. The test will use a dynamically-created test node with this attribute.

## Resolved Dependencies

None. This task introduces no new external dependencies. It uses only:
- Existing Python stdlib (`os`, `logging`)
- Existing internal modules (`worker.ipc`, `worker.nodes.base`, `worker.nodes`)
- Existing `WorkerEvent::Progress` Rust type (already in `anvilml-ipc`)

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| (none) | (none)  | n/a             | n/a            | n/a                    |

## Approach

1. **Add `EMITS_PROGRESS` attribute to `BaseNode`** in `worker/nodes/base.py`:
   - Add class attribute `EMITS_PROGRESS: bool = False` to the `BaseNode` class (after the existing `OUTPUT_SLOTS` attribute).
   - Default is `False` so existing nodes (SaveImage, etc.) are unaffected.
   - No docstring needed for a simple bool attribute — it follows the existing pattern of `NODE_TYPE`, `CATEGORY`, etc.

2. **Modify `run_graph()` in `worker/executor.py`** to emit Progress events:
   - After `result = node.execute(**resolved_inputs)` (line 208) and before `outputs[node_id] = result` (line 209), insert Progress emission logic:
     - Check `if getattr(node_cls, "EMITS_PROGRESS", False):`
     - If mock mode (`os.environ.get("ANVILML_WORKER_MOCK") == "1"`): emit 3 Progress events with `step=1`, `step=2`, `step=3`, all with `total_steps=3` and `preview_b64=None`
     - If not mock mode: emit 1 Progress event with `step=1`, `total_steps=1`, `preview_b64=None`
     - Each Progress event dict: `{"_type": "Progress", "job_id": ctx.job_id, "step": N, "total_steps": T, "preview_b64": None}`
   - Add `import os` at the top of `executor.py` (it is not currently imported — only `logging`, `collections.deque`, and `typing.Any` are imported).

3. **Verify `worker_main.py`** already passes `emit=send_event`:
   - Line 226 already has `emit=send_event` in the `NodeContext` constructor call. No changes needed.
   - Document this in the plan as confirmed.

4. **Add test in `worker/tests/test_executor.py`**:
   - Create a new test function `test_progress_events_emitted_in_mock_mode()` that:
     - Creates a mock node class with `EMITS_PROGRESS = True` using `_make_node_class()`
     - Builds a graph with a single node of this type
     - Calls `run_graph()` with a `mock_context` fixture (which has an emit capture)
     - Asserts that exactly 3 Progress events were emitted in order (step=1, 2, 3)
     - Asserts each Progress event has correct fields: `_type="Progress"`, `job_id="test-job-1"`, correct `step`, `total_steps=3`, `preview_b64=None`
     - The test runs under the existing `mock_context` fixture which provides a captured emit, and under `conftest.py` which sets `ANVILML_WORKER_MOCK=1`

5. **No changes to `worker_main.py`** are needed — it already passes `emit=send_event` into `NodeContext` at line 226.

## Public API Surface

| Item | Path | Description |
|------|------|-------------|
| New attribute | `worker/nodes/base.py::BaseNode::EMITS_PROGRESS` | `bool = False` class attribute on BaseNode; nodes set this to `True` to declare they emit Progress events during execution |
| Modified function | `worker/executor.py::run_graph` | After each node execute, checks `EMITS_PROGRESS` and emits Progress events via `ctx.emit` |
| New test | `worker/tests/test_executor.py::test_progress_events_emitted_in_mock_mode` | Verifies 3 Progress events emitted in order for a mock step-based node |

No new `pub` items in Rust. The existing `WorkerEvent::Progress` and `WsEvent::JobProgress` types are unchanged.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/base.py` | Add `EMITS_PROGRESS: bool = False` class attribute to `BaseNode` |
| Modify | `worker/executor.py` | Add `import os`; add Progress emission logic in `run_graph()` after each node execute |
| Modify | `worker/tests/test_executor.py` | Add `test_progress_events_emitted_in_mock_mode()` test function |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_executor.py` | `test_progress_events_emitted_in_mock_mode` | A node with `EMITS_PROGRESS=True` causes the executor to emit exactly 3 Progress events in order (step=1,2,3) before ImageReady, in mock mode | `ANVILML_WORKER_MOCK=1` (from conftest.py autouse fixture); NODE_REGISTRY cleared (from registry_clean autouse fixture) | Graph with single node of type "ProgressNode" that has `EMITS_PROGRESS=True` | 3 Progress events captured by emit capture, each with `_type="Progress"`, correct step/total_steps/preview_b64 fields | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_executor.py -v` exits 0 |

## CI Impact

No CI changes required. The existing CI jobs (`worker-linux`, `worker-windows`) already run `pytest worker/tests/` under `ANVILML_WORKER_MOCK=1`. The new test is picked up automatically by the existing pytest command. No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. The changes are pure Python logic with no platform-specific code paths. The `os.environ.get("ANVILML_WORKER_MOCK")` check works identically on Linux and Windows. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `os.environ.get("ANVILML_WORKER_MOCK")` check in `run_graph()` may not be reliable if the env var is set to `"0"` or `"true"` instead of `"1"` | Low | Medium | Use the exact string comparison `== "1"` as done elsewhere in the codebase (see `worker_main.py` line 128). This matches the established convention. |
| Adding `import os` to `executor.py` may introduce a dependency that was previously avoided (the module only used stdlib `logging`, `collections`, `typing`) | Low | Low | `os` is stdlib — no new external dependency. The module already imports stdlib modules. This is a minimal addition. |
| The test node with `EMITS_PROGRESS=True` may not be properly isolated between tests if `NODE_REGISTRY` is not cleared | Low | Medium | The existing `registry_clean` autouse fixture clears `NODE_REGISTRY` before each test, which is the established pattern. The `_make_node_class()` helper uses `@register` which stores in `NODE_REGISTRY`, so the fixture handles isolation. |
| Progress events emitted during non-mock mode may have incorrect step numbering if a node has multiple internal steps | Medium | Low | For the initial implementation, non-mock mode emits a single Progress event with `step=1, total_steps=1`. The step numbering can be refined in a follow-up task when concrete step-based nodes (Sampler) are implemented. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_executor.py -v` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v` exits 0 (full Python test suite)
- [ ] `grep -n "EMITS_PROGRESS" worker/nodes/base.py` returns at least one line (attribute exists)
- [ ] `grep -n "Progress" worker/executor.py` returns at least one line (Progress emission logic exists)
- [ ] `grep -n "emit.*send_event" worker/worker_main.py` returns at least one line (emit wiring confirmed)
- [ ] `head -1 .forge/reports/P16-A1_plan.md` prints `# Plan Report: P16-A1`
- [ ] `grep "^## " .forge/reports/P16-A1_plan.md` shows 12 section headings
- [ ] `wc -l .forge/reports/P16-A1_plan.md` returns a value > 40
