# Plan Report: P14-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P14-A2                                        |
| Phase       | 014 — Dispatch & Mock Execute               |
| Description | anvilml-worker: mock execute in worker_main.py and executor.py |
| Depends on  | P14-A1 (dispatch loop)                        |
| Project     | anvilml                                       |
| Planned at  | 2026-06-20T10:30:00Z                          |
| Attempt     | 1                                             |

## Objective

Implement the Python-side job execution pipeline for the AnvilML worker: a `run_graph()` function in `worker/executor.py` that topologically sorts and executes nodes from a graph JSON payload, a mock `SaveImage` node in `worker/nodes/image.py` that emits a 64×64 black PNG as base64, and an Execute message handler in `worker/worker_main.py` that orchestrates the execution and reports `Completed` or `Failed` back to the Rust supervisor. This closes the worker-side half of the job dispatch loop started by P14-A1.

## Scope

### In Scope
- Create `worker/executor.py` with `run_graph(graph: dict, settings: dict, ctx: NodeContext) -> None`
  - Topological sort of nodes from graph JSON
  - Input resolution from prior node outputs
  - Node instantiation from `NODE_REGISTRY`
  - Execute call and output storage
- Create `worker/nodes/image.py` with `SaveImage` node
  - Mock-mode: generate 64×64 black PNG via stdlib (struct, base64, zlib)
  - Emit `ImageReady { job_id, image_b64, width, height }` via `ctx.emit`
  - No PIL/torch/diffusers imports in mock path
- Modify `worker/worker_main.py` to handle `Execute` message
  - Record start time with `time.monotonic()`
  - Call `run_graph()` with graph, settings, and a `NodeContext`
  - Send `Completed { job_id, elapsed_ms }` on success
  - Send `Failed { job_id, error }` on node failure
- Create `worker/tests/test_executor.py` with ≥ 4 tests

### Out of Scope
- Real-hardware node implementations (LoadModel, Sampler, etc.) — added in later phases
- Artifact file system storage — Phase 015
- Dispatch loop background task — P14-A1
- Completed/Failed event handling in Rust scheduler — P14-A3
- Cancellation support within `run_graph` — future task

## Existing Codebase Assessment

The Python worker at `worker/worker_main.py` already has a complete startup sequence: it reads env vars, connects the ZeroMQ DEALER socket via `ipc.connect()`, imports all registered node types via `_import_nodes()`, builds the `Ready` event with node type descriptors from `NODE_REGISTRY`, sends the Ready event, and enters a dispatch loop handling `Ping` → `Pong` and `Shutdown` → exit 0. The `Execute` message type is already defined on the Rust side (`WorkerMessage::Execute { job_id, graph, settings, device_index }`) but handled as "unknown" in the worker.

The node registration infrastructure (`worker/nodes/base.py`) provides `BaseNode` (ABC with `execute(**inputs) -> dict`), `NodeContext` (job_id, device, cancel_flag, emit, pipeline_cache), `SlotSpec`, and the `@register` decorator. `NODE_REGISTRY` is a module-level dict in `worker/nodes/__init__.py`, populated by auto-import of sibling `.py` files. No concrete node modules exist yet — only `base.py`.

The IPC module (`worker/ipc.py`) provides `connect(port, worker_id)`, `send_event(data)`, and `recv_message()` using ZeroMQ DEALER sockets and msgpack serialization. Tests follow a pattern of creating isolated zmq sockets per test, with `_reset_ipc_state()` helpers for module-level state cleanup.

The test suite uses `pytest` with `conftest.py` providing an autouse `mock_mode` fixture that sets `ANVILML_WORKER_MOCK=1`. Tests import directly from the modules under test and use standard `assert` statements. The existing `test_nodes_base.py` demonstrates the pattern for node tests: clearing `NODE_REGISTRY` via a fixture, defining inline test node classes with `@register`, and asserting registry contents.

No gap between design doc and current source affects this task: the `NodeContext` class already has the `emit` callable field, `NODE_REGISTRY` is populated at import time, and the `Execute` message format matches what the task requires.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| python | msgpack | 1.2.1           | pypi-query MCP | n/a                    |
| python | pyzmq   | 27.1.0          | pypi-query MCP | n/a                    |

Both `msgpack` and `pyzmq` are already declared in `worker/requirements/base.txt` and compatible with Python 3.12 (the project's required version). No new external dependencies are introduced — the `SaveImage` mock uses only Python stdlib (`struct`, `base64`, `zlib`, `io`).

## Approach

1. **Create `worker/executor.py`** with the `run_graph` function:
   - Implement a topological sort using Kahn's algorithm on the graph JSON. The graph is expected as a dict with a `"nodes"` key containing a list of node objects. Each node object has `"id"`, `"type"`, and `"inputs"` keys. Input values that are lists (e.g. `["1", "latent"]`) reference another node's output by (node_id, output_name). Build a dependency graph: for each node, collect all node IDs it depends on from its input references. Perform Kahn's algorithm to produce a sorted execution order. If a cycle is detected, raise `ValueError("graph contains a cycle")`.
   - For each node in topo order: instantiate the node class from `NODE_REGISTRY[node_type]` with a `NodeContext`; resolve input values by looking up the referenced node outputs; call `node.execute(**inputs)`; store outputs keyed by node id.
   - The function signature: `def run_graph(graph: dict, settings: dict, ctx: NodeContext) -> None`.
   - Add a Google-style docstring to the module and function.

2. **Create `worker/nodes/image.py`** with the `SaveImage` node:
   - Define `class SaveImage(BaseNode)` with metadata: `NODE_TYPE = "SaveImage"`, `CATEGORY = "Output"`, appropriate `DISPLAY_NAME` and `DESCRIPTION`.
   - Input slots: one required `IMAGE` slot named `"image"`. No output slots (output is via IPC event, not slot).
   - `execute(**inputs)` implementation (mock mode only):
     - Read `image = inputs["image"]` (will be `None` in mock mode since no upstream nodes exist yet).
     - Generate a minimal 64×64 black PNG using only stdlib: construct the PNG binary using `struct.pack` for the PNG signature, IHDR chunk (64×64, 8-bit RGB), and IDAT chunk (zlib-compressed scanlines of zero bytes). Use `base64.b64encode` to encode the result.
     - Emit `ImageReady` event via `ctx.emit({"_type": "ImageReady", "job_id": self.ctx.job_id, "image_b64": b64, "width": 64, "height": 64})`.
     - Return empty dict (no output slots).
   - Decorate with `@register` so auto-import in `__init__.py` picks it up.
   - Add Google-style docstrings to the class and method.

3. **Modify `worker/worker_main.py`** to handle `Execute` message:
   - In the dispatch loop's `elif msg_type == "Execute":` branch:
     - Extract `job_id`, `graph`, `settings`, `device_index` from the message dict.
     - Record `start = time.monotonic()`.
     - Build a `NodeContext` with `job_id`, `device` (e.g. `"cpu"` or `"cuda:0"` based on `device_index`), a simple cancel flag (e.g. a list `[False]`), `send_event` as the emit callable, and an empty dict for `pipeline_cache`.
     - Call `run_graph(graph, settings, ctx)`.
     - On success: compute `elapsed_ms = int((time.monotonic() - start) * 1000)`; send `Completed { job_id, elapsed_ms }` via `send_event`.
     - On exception: compute `elapsed_ms`; send `Failed { job_id, error: str(e) }` via `send_event`; log the error to stderr.
   - Add `import time` at the top of the file (currently absent).

4. **Create `worker/tests/test_executor.py`** with ≥ 4 tests:
   - **test_run_graph_topo_order**: Define a mock node class with inputs/outputs, build a graph with two nodes where node 2 depends on node 1, call `run_graph`, assert outputs are stored in correct order.
   - **test_saveimage_emits_image_ready**: Import SaveImage, build a graph with a single SaveImage node, call `run_graph` with a mock context that captures emit calls, assert an `ImageReady` event was emitted with correct fields (job_id, image_b64 as 64×64 PNG, width=64, height=64).
   - **test_completed_sent_after_run_graph**: Integration-style test: build a mock graph, call `run_graph`, assert the function returns without error (simulating Completed path).
   - **test_failed_sent_on_node_error**: Define a node that raises in `execute()`, build a graph with that node, call `run_graph`, assert a `ValueError` is raised (simulating Failed path).

## Public API Surface

| Item | Module Path | Signature / Definition |
|------|-------------|----------------------|
| Function | `worker.executor.run_graph` | `def run_graph(graph: dict, settings: dict, ctx: NodeContext) -> None` |
| Class | `worker.nodes.image.SaveImage` | `class SaveImage(BaseNode): NODE_TYPE = "SaveImage", CATEGORY = "Output", DISPLAY_NAME = "Save Image", DESCRIPTION = "...", INPUT_SLOTS = [SlotSpec("image", "IMAGE")], OUTPUT_SLOTS = [], def execute(self, **inputs: Any) -> dict[str, Any]` |
| New import | `worker.nodes.image` | Auto-imported by `__init__.py` via `_ensure_imported()` |

Note: `NodeContext` already exists in `worker.nodes.base` (no changes needed). The `run_graph` function is module-private (no `pub` equivalent in Python — not exported in `__all__`), but it is the primary entry point for execution.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/executor.py` | Topological-sort node executor with `run_graph()` function |
| CREATE | `worker/nodes/image.py` | `SaveImage` node with mock PNG generation |
| MODIFY | `worker/worker_main.py` | Add `time` import; add `Execute` message handler in dispatch loop |
| CREATE | `worker/tests/test_executor.py` | ≥ 4 tests for executor and SaveImage |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_executor.py` | `test_run_graph_topo_order` | `run_graph` executes nodes in correct topological order, resolving inputs from prior outputs | NODE_REGISTRY has a test node registered; graph has 2 nodes with dependency | Graph with node A → node B dependency | Outputs dict contains both nodes' results, B's inputs include A's outputs | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_executor.py::test_run_graph_topo_order -v` exits 0 |
| `worker/tests/test_executor.py` | `test_saveimage_emits_image_ready` | SaveImage node generates 64×64 black PNG and emits ImageReady event | SaveImage is registered; NodeContext with captured emit callable | Graph with single SaveImage node | ImageReady event emitted with correct job_id, image_b64 (valid PNG), width=64, height=64 | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_executor.py::test_saveimage_emits_image_ready -v` exits 0 |
| `worker/tests/test_executor.py` | `test_completed_sent_after_run_graph` | `run_graph` returns normally on successful execution (Completed path) | NODE_REGISTRY has a test node; graph is valid | Graph with single no-op node | No exception raised; function returns None | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_executor.py::test_completed_sent_after_run_graph -v` exits 0 |
| `worker/tests/test_executor.py` | `test_failed_sent_on_node_error` | `run_graph` raises exception when a node's execute() fails (Failed path) | NODE_REGISTRY has a failing test node | Graph with one node that raises in execute() | ValueError raised with error message from node | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_executor.py::test_failed_sent_on_node_error -v` exits 0 |

## CI Impact

The new `worker/tests/test_executor.py` test file is automatically picked up by the existing `worker-linux` and `worker-windows` CI jobs which run `pytest worker/tests/`. No CI workflow changes are needed. The test file uses only stdlib and already-declared dependencies (msgpack, pyzmq).

## Platform Considerations

None identified. The executor uses only Python stdlib (`struct`, `base64`, `zlib`, `collections`, `time`) and the existing `worker.nodes` infrastructure. The PNG generation via `struct.pack` is platform-neutral. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Graph JSON schema mismatch: the actual graph format from the Rust `serde_json::Value` may differ from the assumed `{"nodes": [...]}` structure. The Rust side serialises `graph` as `serde_json::Value` which becomes a Python dict via msgpack. | Medium | High | The graph format is defined by the scheduler in P14-A1. Since P14-A1 runs first, the executor should be designed with a clear, documented schema. The topo sort implementation must handle the actual field names used. |
| SaveImage PNG generation produces an invalid PNG file that downstream consumers reject. The mock PNG must be a valid PNG binary with correct CRC checksums in IHDR and IDAT chunks. | Low | Medium | Use `struct.pack` with correct PNG magic bytes (`\x89PNG\r\n\x1a\n`), proper chunk lengths, and let Python's `zlib.crc32` compute checksums automatically. The minimal 64×64 RGB PNG has a well-known binary layout. |
| Topological sort fails on graphs with cycles, producing infinite loops or incorrect ordering. | Low | High | Implement Kahn's algorithm which naturally detects cycles (if not all nodes are processed, a cycle exists). Raise `ValueError` with a clear message. |
| NODE_REGISTRY is empty during tests because no node modules are imported. The test fixture clears the registry but the SaveImage module may not be auto-imported in test scope. | Medium | Medium | The test should explicitly import `worker.nodes.image` to ensure SaveImage is registered. The `registry_clean` fixture from `test_nodes_base.py` clears the registry, so each test must re-register its nodes. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_executor.py -v` exits 0 with ≥ 4 tests
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -c "from worker.executor import run_graph; print('import ok')"` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -c "from worker.nodes.image import SaveImage; print(SaveImage.NODE_TYPE)"` exits 0 and prints `SaveImage`
- [ ] `worker/executor.py` contains a topological sort that orders nodes by dependency (not array order)
- [ ] `worker/nodes/image.py` SaveImage `execute()` generates a valid 64×64 PNG using only stdlib (no PIL, torch, or diffusers imports)
- [ ] `worker/worker_main.py` contains an `Execute` message handler that calls `run_graph` and sends `Completed` or `Failed`
