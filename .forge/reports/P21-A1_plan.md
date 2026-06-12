# Plan Report: P21-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P21-A1                                      |
| Phase       | 021 — Real Python Worker — ZiT              |
| Description | worker: nodes/base.py BaseNode + NodeContext + NODE_REGISTRY + @register |
| Depends on  | P20-A4                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-06-12T17:12:00Z                        |
| Attempt     | 1                                           |

## Objective

Create the foundational node infrastructure for the AnvilML Python worker: a `NodeContext` dataclass, an abstract `BaseNode` class with class-level slot declarations, a module-level `NODE_REGISTRY` dict, and a `@register` decorator that auto-populates the registry. Also update `worker/nodes/__init__.py` to import node modules so the registry is populated at worker startup.

## Scope

### In Scope
- Create `worker/nodes/base.py` with:
  - `NodeContext` dataclass: `pipeline_cache`, `device_str`, `emit_fn`, `cancel_flag: threading.Event`, `job_id: str`
  - `BaseNode` ABC with `ClassVar NODE_TYPE: str`, `INPUT_SLOTS: list[str]`, `OUTPUT_SLOTS: list[str]`; `__init__(self, ctx: NodeContext)`; abstract `execute(self, **inputs) -> dict[str, Any]`
  - Module-level `NODE_REGISTRY: dict[str, type[BaseNode]] = {}`
  - `@register` decorator that stores the class in `NODE_REGISTRY` keyed by `NODE_TYPE`
- Update `worker/nodes/__init__.py` to import every `.py` module in the package (except `base.py`) so node classes self-register on import
- Create `worker/tests/test_nodes_base.py` with two tests:
  1. `@register` populates `NODE_REGISTRY` — register a dummy node, assert `NODE_REGISTRY` contains it
  2. Missing `execute` raises `TypeError` — subclass without implementing `execute` cannot be instantiated

### Out of Scope
- Any concrete node implementations (ZiT, SDXL, common) — these are P21-A5
- The executor (`executor.py`) — this is P21-A2
- Pipeline cache — this is P21-A3
- Defaults and requirements — this is P21-A4
- Parity test with Rust `KNOWN_NODE_TYPES` — this is P21-A6
- Any Rust-side changes

## Approach

1. **Write `worker/nodes/base.py`:**
   - Import `ABC`, `ClassVar`, `dataclass`, `Any`, `dict`, `threading.Event` from appropriate modules
   - Define `NodeContext` as a `@dataclass` with fields: `pipeline_cache`, `device_str: str`, `emit_fn`, `cancel_flag: threading.Event`, `job_id: str`
   - Define `BaseNode(ABC)` with class variables `NODE_TYPE: ClassVar[str] = ""`, `INPUT_SLOTS: ClassVar[list[str]] = []`, `OUTPUT_SLOTS: ClassVar[list[str]] = []`; `__init__(self, ctx: NodeContext)` storing `self.ctx = ctx`; `@abstractmethod def execute(self, **inputs) -> dict[str, Any]`
   - Define `NODE_REGISTRY: dict[str, type[BaseNode]] = {}`
   - Define `def register(cls: type[BaseNode]) -> type[BaseNode]: NODE_REGISTRY[cls.NODE_TYPE] = cls; return cls`

2. **Update `worker/nodes/__init__.py`:**
   - Add import of `worker.nodes.base` (or `from . import base`) to ensure `NODE_REGISTRY` is defined
   - Use a glob or explicit imports of remaining modules (`common`, `zit`, `sdxl`) — since only `base.py` exists now, the `__init__.py` should import them conditionally or use `pkgutil`/`importlib` to auto-discover. Per the design doc (§14.3), `nodes/__init__.py` imports every node module so the registry is populated at startup. Use a simple approach: import `base` explicitly, then use `pkgutil.iter_modules` to import all other `.py` modules in the package directory.

3. **Create `worker/tests/test_nodes_base.py`:**
   - Test 1: `test_register_populates_registry` — define a concrete subclass of `BaseNode` with a dummy `NODE_TYPE`, decorate it with `@register`, assert `NODE_REGISTRY` contains the key and the class
   - Test 2: `test_missing_execute_raises_typeerror` — define an incomplete subclass of `BaseNode` (no `execute` override), attempt to instantiate it, assert `TypeError` is raised

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `worker/nodes/base.py` | NodeContext dataclass, BaseNode ABC, NODE_REGISTRY, @register decorator |
| Modify | `worker/nodes/__init__.py` | Auto-import node modules to populate registry |
| Create | `worker/tests/test_nodes_base.py` | Tests for @register and missing execute |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `worker/tests/test_nodes_base.py` | `test_register_populates_registry` | `@register` decorator adds class to `NODE_REGISTRY` keyed by `NODE_TYPE` |
| `worker/tests/test_nodes_base.py` | `test_missing_execute_raises_typeerror` | Subclass without `execute` cannot be instantiated (TypeError) |

## CI Impact

No CI workflow files are modified. The new test file is picked up automatically by `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v` (ENVIRONMENT.md §6, Gate for Python worker). No Rust changes, so no `cargo test` or `cargo clippy` changes needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `pkgutil` auto-import in `__init__.py` fails if module has import errors | Low | Medium | Use explicit imports guarded by `try/except ImportError`; fall back to only importing `base` |
| `NODE_REGISTRY` gets polluted by test runs (shared module-level dict) | Medium | Low | Import fresh module per test or clear `NODE_REGISTRY` in a `pytest.fixture` teardown |
| `threading.Event` type hint mismatch across Python versions | Low | Low | Use `threading.Event` directly; Python 3.12+ handles this cleanly |

## Acceptance Criteria

- [ ] `worker/nodes/base.py` exists with `NodeContext` dataclass, `BaseNode` ABC, `NODE_REGISTRY`, and `@register` decorator
- [ ] `worker/nodes/__init__.py` imports `base` and auto-discovers other node modules
- [ ] `worker/tests/test_nodes_base.py` exists and both tests pass
- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_nodes_base.py -v` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v` exits 0 (no regression)
