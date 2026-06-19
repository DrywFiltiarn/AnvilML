# Plan Report: P11-B1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P11-B1                                        |
| Phase       | 011 — Dynamic Node Registry                 |
| Description | worker/nodes/__init__.py: NODE_REGISTRY auto-import |
| Depends on  | none                                          |
| Project     | anvilml                                       |
| Planned at  | 2026-06-19T16:15:00Z                          |
| Attempt     | 1                                             |

## Objective

Establish the Python-side node registration infrastructure: a `NODE_REGISTRY` dict in `worker/nodes/__init__.py` populated by auto-importing all `.py` files via `pkgutil.iter_modules`, a `@register` decorator and `BaseNode` ABC in `worker/nodes/base.py`, and wiring into `worker/worker_main.py` so that `_import_nodes()` triggers the auto-import and builds the `node_types` list for the `Ready` IPC event. The observable outcome is that `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_base.py` exits 0 with ≥ 3 tests.

## Scope

### In Scope
- Create `worker/nodes/` package directory with `__init__.py` and `base.py`
- `worker/nodes/__init__.py`: `NODE_REGISTRY: dict[str, type] = {}` global; `_ensure_imported()` idempotent function using `pkgutil.iter_modules(__path__)`; module-level call to `_ensure_imported()` at import time
- `worker/nodes/base.py`: `@register` decorator (validates `NODE_TYPE`, `CATEGORY`, `DISPLAY_NAME`, `DESCRIPTION`, `INPUT_SLOTS`, `OUTPUT_SLOTS`); `SlotSpec` dataclass (`name: str`, `slot_type: str`, `optional: bool = False`); `NodeContext` class (`job_id`, `device`, `cancel_flag`, `emit`, `pipeline_cache`); `BaseNode` ABC with `__init__(self, ctx: NodeContext)` and abstract `execute(self, **inputs: Any) -> dict[str, Any]`
- `worker/worker_main.py`: add `_import_nodes()` function; update `main()` to call `_import_nodes()` before building the Ready event; build `node_types` list from `NODE_REGISTRY` entries, converting each to a `NodeTypeDescriptor`-compatible dict
- Create `worker/tests/test_nodes_base.py` with ≥ 3 unit tests

### Out of Scope
- Actual node implementations (LoadModel, Sampler, etc.) — these are added in later phases (P14, P18)
- Rust-side `NodeTypeRegistry` — handled by P11-A1
- `GET /v1/nodes` endpoint — handled by P11-A3
- Architecture dispatch system (`worker/nodes/arch/`) — handled in later phases
- Any changes to Rust crates, Cargo.toml, or CI configuration

## Existing Codebase Assessment

The `worker/nodes/` directory does not exist — this task creates the entire Python node package from scratch. The `worker/worker_main.py` module already has a complete mock-mode `main()` function that connects to the Rust ROUTER socket, sends a `Ready` event with `node_types: []`, and enters a message dispatch loop. The `worker/ipc.py` module provides `connect()`, `send_event()`, and `recv_message()` using ZeroMQ DEALER sockets with msgpack serialization.

Existing test patterns in `worker/tests/` (e.g., `test_worker_main.py`) use subprocess spawning with ZeroMQ ROUTER sockets for integration tests, while `conftest.py` provides an autouse `mock_mode` fixture that sets `ANVILML_WORKER_MOCK=1` and restores the original value unconditionally. The design doc (`ANVILML_DESIGN.md §13.4`) provides the exact reference implementation for `NODE_REGISTRY`, `@register`, `SlotSpec`, `NodeContext`, and `BaseNode`.

The `WorkerEvent::Ready` Rust struct (in `crates/anvilml-ipc/src/messages.rs`) declares `node_types: Vec<NodeTypeDescriptor>` where `NodeTypeDescriptor` has fields `type_name`, `display_name`, `category`, `description`, `inputs`, `outputs`. The Python worker sends this as a plain dict via msgpack, so the dict keys must match these Rust field names exactly.

## Resolved Dependencies

None. This task uses only Python standard library modules (`pkgutil`, `abc`, `dataclasses`, `typing`) that are part of the base Python 3.12 installation. No external packages are introduced.

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| stdlib | pkgutil | 3.12            | n/a            | n/a                    |
| stdlib | abc     | 3.12            | n/a            | n/a                    |
| stdlib | dataclasses | 3.12        | n/a            | n/a                    |

## Approach

1. **Create `worker/nodes/` directory.**
   - `mkdir -p worker/nodes` — creates the package directory.

2. **Create `worker/nodes/base.py`** with the following components, derived from the design doc (§13.4):
   - **`SlotSpec` dataclass**: `@dataclass class SlotSpec: name: str; slot_type: str; optional: bool = False`. This declares one input or output slot on a node. The `slot_type` is a string that must match a `SlotType` enum value (e.g. `"MODEL"`, `"CLIP"`).
   - **`NodeContext` class**: A runtime context with attributes `job_id: str`, `device: str`, `cancel_flag`, `emit`, `pipeline_cache`. The `emit` attribute is a `Callable` for emitting `WorkerEvent` dicts back to the supervisor.
   - **`BaseNode` ABC**: `class BaseNode(ABC)` with class attributes `NODE_TYPE: str = ""`, `CATEGORY: str = ""`, `DISPLAY_NAME: str = ""`, `DESCRIPTION: str = ""`, `INPUT_SLOTS: list[SlotSpec] = []`, `OUTPUT_SLOTS: list[SlotSpec] = []`. Constructor `__init__(self, ctx: NodeContext)` stores `self.ctx = ctx`. Abstract method `execute(self, **inputs: Any) -> dict[str, Any]`.
   - **`@register` decorator**: `def register(cls: type) -> type` that validates the class has all six required attributes (`NODE_TYPE`, `CATEGORY`, `DISPLAY_NAME`, `DESCRIPTION`, `INPUT_SLOTS`, `OUTPUT_SLOTS`), raises `TypeError` if any is missing, then sets `NODE_REGISTRY[cls.NODE_TYPE] = cls` and returns `cls`.

3. **Create `worker/nodes/__init__.py`** with:
   - `from worker.nodes.base import NODE_REGISTRY` — re-export the registry from base.py (or define it here and import from base).
   - Actually, the design doc shows `NODE_REGISTRY` defined in the same file as `@register` and `BaseNode`. But the task description says `worker/nodes/__init__.py` contains `NODE_REGISTRY` and auto-import, while `worker/nodes/base.py` contains `@register`, `BaseNode`, `SlotSpec`, `NodeContext`. I'll put `NODE_REGISTRY` in `__init__.py` and import it into `base.py` so the decorator can write to it. Or more cleanly: define `NODE_REGISTRY` in `__init__.py`, import `NODE_REGISTRY` in `base.py` for the `@register` decorator to use.
   - `_ensure_imported()` function: uses `pkgutil.iter_modules(__path__)` to iterate over all submodules, imports each via `importlib.import_module`, catches `ImportError` and logs a warning (using `print` to stderr since no logging module is set up yet), sets a module-level `_imported: bool = False` flag for idempotency.
   - Call `_ensure_imported()` at module level so auto-import happens on first import.

4. **Update `worker/worker_main.py`**:
   - Add `_import_nodes()` function that does `import worker.nodes` (which triggers the auto-import in `__init__.py`).
   - Add `_build_node_types_list()` helper that iterates `NODE_REGISTRY`, builds a list of dicts matching `NodeTypeDescriptor` shape: `{type_name, display_name, category, description, inputs, outputs}` where `inputs`/`outputs` are lists of `{name, slot_type, optional}` dicts converted from `SlotSpec` objects.
   - In `main()`, call `_import_nodes()` before building the Ready event. Replace `node_types: []` with the result of `_build_node_types_list()`.

5. **Create `worker/tests/test_nodes_base.py`** with ≥ 3 tests:
   - `test_registry_populated_after_import`: imports `worker.nodes`, asserts `NODE_REGISTRY` is accessible (will be empty since no concrete nodes exist yet — this verifies the import path works without errors).
   - `test_register_decorator_adds_class`: creates a concrete test node class with all required attributes, applies `@register`, asserts it appears in `NODE_REGISTRY` with the correct key.
   - `test_base_node_cannot_be_instantiated`: attempts `BaseNode()` directly, asserts `TypeError` is raised (ABC enforcement).
   - `test_slot_spec_dataclass`: verifies `SlotSpec` creates a dataclass with correct fields and defaults.

6. **Verification**: Run the acceptance command.

## Public API Surface

| Item | Path | Signature / Description |
|------|------|------------------------|
| `NODE_REGISTRY` | `worker/nodes/__init__.py` | `NODE_REGISTRY: dict[str, type] = {}` — module-level global dict mapping node type name to class |
| `register` | `worker/nodes/base.py` | `def register(cls: type) -> type` — decorator that validates required attrs and adds to NODE_REGISTRY |
| `SlotSpec` | `worker/nodes/base.py` | `@dataclass class SlotSpec: name: str; slot_type: str; optional: bool = False` |
| `NodeContext` | `worker/nodes/base.py` | `class NodeContext: job_id: str; device: str; cancel_flag; emit; pipeline_cache` |
| `BaseNode` | `worker/nodes/base.py` | `class BaseNode(ABC): NODE_TYPE, CATEGORY, DISPLAY_NAME, DESCRIPTION, INPUT_SLOTS, OUTPUT_SLOTS; __init__(self, ctx: NodeContext); abstract execute(self, **inputs: Any) -> dict[str, Any]` |
| `_import_nodes` | `worker/worker_main.py` | `def _import_nodes() -> None` — imports `worker.nodes` to trigger auto-import |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/__init__.py` | NODE_REGISTRY global; auto-import via pkgutil.iter_modules |
| CREATE | `worker/nodes/base.py` | @register decorator; BaseNode ABC; SlotSpec dataclass; NodeContext class |
| MODIFY | `worker/worker_main.py` | Add _import_nodes(); build node_types list from NODE_REGISTRY for Ready event |
| CREATE | `worker/tests/test_nodes_base.py` | Unit tests for registry, decorator, ABC enforcement, SlotSpec |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_nodes_base.py` | `test_registry_populated_after_import` | Importing `worker.nodes` does not raise; `NODE_REGISTRY` is accessible | `worker/nodes/` package exists | `import worker.nodes` | No exception; `NODE_REGISTRY` is a dict | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_base.py::test_registry_populated_after_import` exits 0 |
| `worker/tests/test_nodes_base.py` | `test_register_decorator_adds_class` | `@register` decorator adds class to `NODE_REGISTRY` with correct key | `NODE_REGISTRY` is empty (or test cleans up) | A class with all 6 required attributes decorated with `@register` | `NODE_REGISTRY["TestNode"]` returns the class | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_base.py::test_register_decorator_adds_class` exits 0 |
| `worker/tests/test_nodes_base.py` | `test_base_node_cannot_be_instantiated` | `BaseNode()` raises `TypeError` (ABC enforcement) | `BaseNode` is importable | `BaseNode()` call | `TypeError` raised | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_base.py::test_base_node_cannot_be_instantiated` exits 0 |
| `worker/tests/test_nodes_base.py` | `test_slot_spec_dataclass` | `SlotSpec` is a dataclass with correct fields and defaults | `SlotSpec` is importable | `SlotSpec("input1", "MODEL")` | Dataclass instance with `name="input1"`, `slot_type="MODEL"`, `optional=False` | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_base.py::test_slot_spec_dataclass` exits 0 |

## CI Impact

No CI changes required. The new test file `worker/tests/test_nodes_base.py` is automatically picked up by the existing `worker-linux` and `worker-windows` CI jobs which run `pytest worker/tests/ -v`. No new CI gates or workflow files are modified.

## Platform Considerations

None identified. `pkgutil.iter_modules` and `abc.ABC` are cross-platform standard library modules. The `worker/nodes/` package is a pure Python module with no platform-specific code. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `pkgutil.iter_modules(__path__)` returns submodule names without `.py` extension; `importlib.import_module()` needs the full dotted module path including `worker.nodes.` prefix. Forgetting the prefix will cause `ModuleNotFoundError`. | Medium | High | Build the full module name as `f"worker.nodes.{mod_name}"` before calling `importlib.import_module()`. Write a test that verifies the import path resolves correctly. |
| Auto-import at module level (`_ensure_imported()` called in `__init__.py` body) runs before `NODE_REGISTRY` is fully set up if there's a circular import between `__init__.py` and `base.py`. | Low | High | Define `NODE_REGISTRY` in `__init__.py`, import it in `base.py` for the `@register` decorator. The auto-import in `__init__.py` only imports sibling modules (not `base.py`), so no circular import occurs. |
| The `_build_node_types_list()` function in `worker_main.py` must produce dicts with keys matching `NodeTypeDescriptor` Rust struct fields (`type_name`, `display_name`, `category`, `description`, `inputs`, `outputs`). A key mismatch will cause the Rust side to fail deserialization. | Medium | High | Use exact Rust field names as dict keys. Each `SlotSpec` converts to `{"name": ..., "slot_type": ..., "optional": ...}`. Test the dict structure in isolation. |
| `NodeContext.__init__` takes `cancel_flag` and `emit` without type annotations (the design doc uses bare `cancel_flag` and `emit`). Without `from __future__ import annotations`, these become runtime types. | Low | Low | Use `from __future__ import annotations` at the top of `base.py` so all annotations are strings and evaluated lazially. This is already imported by every file in the worker. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_base.py -v` exits 0 with ≥ 3 tests passing
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -c "import worker.nodes; print(type(worker.nodes.NODE_REGISTRY))"` prints `<class 'dict'>`
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -c "from worker.nodes.base import BaseNode; BaseNode()"` exits non-zero (TypeError)
- [ ] `worker/nodes/__init__.py` exists and contains `pkgutil.iter_modules`
- [ ] `worker/nodes/base.py` exists and contains `BaseNode`, `SlotSpec`, `NodeContext`, `@register`
- [ ] `worker/worker_main.py` contains `_import_nodes` function
