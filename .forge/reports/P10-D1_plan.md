# Plan Report: P10-D1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P10-D1                                        |
| Phase       | 10 — Generic Node Groundwork                  |
| Description | worker_main.py: wire real _import_nodes() to worker.nodes auto-import |
| Depends on  | P10-C1                                        |
| Project     | anvilml                                       |
| Planned at  | 2026-07-06T08:30:00Z                          |
| Attempt     | 1                                             |

## Objective

Replace the `_import_nodes()` stub in `worker/worker_main.py` (which returns a hardcoded `[]`) with real logic that triggers `worker.nodes` auto-import and builds `NodeTypeDescriptor`-equivalent dicts from `NODE_REGISTRY`. Both the real-mode and mock-mode call sites use the identical function, so only one code change is needed. The observable result (`node_types` is an empty list) does not change — no concrete node files exist yet — but the wiring is now real, satisfying the Phase 10 goal of closing the loop between `worker_main.py` and the node system.

## Scope

### In Scope
- Modify `worker/worker_main.py`'s `_import_nodes()` function to call `worker.nodes` auto-import and build dicts from `NODE_REGISTRY`.
- The single `_import_nodes()` function is called identically from both `_real_startup_sequence()` (line 134) and `_mock_startup_sequence()` (line 226); one code change satisfies both.
- No new files are created. No new tests are written (the existing test `test_import_nodes_returns_empty_list` in `test_worker_main.py` already validates the empty-list contract, and the `test_nodes_init.py` suite validates the auto-import mechanism itself).
- No Rust changes. No CI changes. No dependency changes.

### Out of Scope
None. This task's `defers_to` field is `[]` — no scope is deferred. The `_import_nodes()` function will return a real (currently-empty) list derived from `NODE_REGISTRY`, not a stub.

## Existing Codebase Assessment

**What exists:** Phase 10's prerequisite tasks (P10-A1 through P10-C1) have already been implemented:
- `worker/nodes/base.py` contains `NODE_REGISTRY`, `@register`, `SlotSpec`, `NodeContext`, and `BaseNode(ABC)`.
- `worker/nodes/__init__.py` contains the auto-import loop (`_import_nodes()`) that uses `pkgutil.iter_modules()` to import `.py` files directly under `nodes/` (skipping `__init__`, `base`, and packages), and calls itself at module load time.
- `worker/nodes/arch/{diffusion,clip,vae}/__init__.py` contain the shared `get_module()` dispatch logic for each family.

**Established patterns:** Python code in this project uses Google-style docstrings, structured logging via `logging.getLogger(__name__)`, and `unittest.mock.patch` for test isolation. The `_import_nodes()` function currently has a simple return-type annotation (`-> list`) that should be tightened to `-> list[dict]`.

**Gap between design doc and current source:** The design doc (§14.2) says `_import_nodes()` "builds Vec<NodeTypeDescriptor> from NODE_REGISTRY." The current source returns `[]` unconditionally. The Rust `NodeTypeDescriptor` struct (design doc §5.6) has fields: `type_name`, `display_name`, `category`, `description`, `inputs`, `outputs`. Each Python node class has class attributes `NODE_TYPE`, `DISPLAY_NAME`, `CATEGORY`, `DESCRIPTION`, `INPUT_SLOTS`, `OUTPUT_SLOTS` that map 1:1 to the Rust struct. The Python `SlotSpec` dataclass (`name`, `slot_type`, `optional`) maps to Rust's `SlotDescriptor` (`name`, `slot_type`, `optional`). At this phase, `NODE_REGISTRY` is empty so the output is `[]`, but the conversion logic must be present and correct so that when concrete nodes are added in later phases, the Ready event carries accurate type descriptors.

## Resolved Dependencies

None. This task uses only Python standard library modules (`os`, `pkgutil`, `importlib.util`) and the project's own `worker.nodes` package — no external crates or PyPI packages are introduced or referenced.

## Approach

1. **Modify `_import_nodes()` in `worker/worker_main.py`** (lines 18-30):
   - Add an import of `worker.nodes` at the top of the function body (not at module level, to avoid transitive torch dependencies — this follows the existing pattern where `worker.ipc` is also imported inside functions).
   - After importing `worker.nodes`, access `worker.nodes.base.NODE_REGISTRY` (imported via the auto-import side-effect).
   - Build a list of dicts by iterating over `NODE_REGISTRY.items()`. For each `(type_name, node_cls)`:
     - Construct a dict with keys: `type_name`, `display_name`, `category`, `description`, `inputs`, `outputs`.
     - `type_name` = `node_cls.NODE_TYPE`
     - `display_name` = `node_cls.DISPLAY_NAME`
     - `category` = `node_cls.CATEGORY`
     - `description` = `node_cls.DESCRIPTION`
     - `inputs` = list of dicts: for each `SlotSpec` in `node_cls.INPUT_SLOTS`, build `{"name": spec.name, "slot_type": spec.slot_type, "optional": spec.optional}`.
     - `outputs` = same transformation applied to `node_cls.OUTPUT_SLOTS`.
   - Return the list.
   - Update the return-type annotation from `-> list` to `-> list[dict]`.
   - Update the docstring to remove the Phase 9 placeholder comment and reflect the new behavior.

   **Rationale for importing `worker.nodes` inside the function (not at module level):** The existing codebase pattern (e.g., `worker.ipc` import inside `_real_startup_sequence()` and `_dispatch_loop()`) avoids module-level imports that could trigger side effects during test collection. Importing `worker.nodes` inside `_import_nodes()` ensures the auto-import runs only when node types are actually needed, matching the startup sequence flow.

   **Rationale for reading `NODE_REGISTRY` from `worker.nodes.base`:** `worker/nodes/__init__.py` re-exports `NODE_REGISTRY` via the auto-import mechanism. However, `worker.nodes.base` is the canonical location where `NODE_REGISTRY` is defined. The `worker.nodes` package's `__init__.py` imports `base` as part of its skip list (line 26: `if mod_name in ("__init__", "base") or is_pkg`), meaning `base` is NOT auto-imported by the loop — it must be explicitly imported. Reading from `worker.nodes.base.NODE_REGISTRY` is the correct and explicit path.

2. **No changes to test files.** The existing test `test_import_nodes_returns_empty_list` (line 478-491 of `test_worker_main.py`) asserts `_import_nodes() == []`, which remains true because `NODE_REGISTRY` is empty. No new tests are needed — the auto-import mechanism is already tested by `test_nodes_init.py`.

## Public API Surface

No new public items are introduced. The only change is to an existing private function (`_import_nodes`) in `worker/worker_main.py`:

| Before | After |
|--------|-------|
| `def _import_nodes() -> list:` | `def _import_nodes() -> list[dict]:` |
| Returns hardcoded `[]` | Returns list of dicts built from `NODE_REGISTRY` |

No changes to `worker.nodes` public API. `NODE_REGISTRY`, `@register`, `SlotSpec`, `NodeContext`, and `BaseNode` are unchanged.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/worker_main.py` | Replace `_import_nodes()` stub with real NODE_REGISTRY-based implementation |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `worker/tests/test_worker_main.py` | `test_import_nodes_returns_empty_list` | `_import_nodes()` returns `[]` (NODE_REGISTRY is empty at this phase) | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py::TestNoTorchImport::test_no_torch_import_on_module_load -v` exits 0 |
| `worker/tests/test_worker_main.py` | `test_real_startup_sends_ready_event` | Ready event carries `node_types=[]` through the real startup path | `worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py -v -m real_mode` exits 0 |
| `worker/tests/test_worker_main.py` | `test_mock_startup_sends_ready_event` | Ready event carries `node_types=[]` through the mock startup path | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py::TestMockProbeCapabilities -v` exits 0 |
| `worker/tests/test_nodes_init.py` | `test_node_registry_empty_after_import` | NODE_REGISTRY is empty after auto-import (no concrete node files) | `worker/.venv/bin/python -m pytest worker/tests/test_nodes_init.py -v` exits 0 |

## CI Impact

No CI changes required. The task modifies only `worker/worker_main.py` and does not add new file types, test modules, or configuration. The existing CI jobs (`worker-linux-mock`, `worker-linux-real`, `worker-windows-mock`, `worker-windows-real`) will exercise the modified code through their existing test runs.

## Platform Considerations

None identified. The modification is purely Python standard library (`os`, `importlib.util`) and project-internal imports. No `#[cfg(...)]` guards, path separators, or line-ending handling is affected. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Importing `worker.nodes.base` at module level in `_import_nodes()` could trigger `worker.nodes.__init__.py`'s auto-import loop, which iterates `pkgutil.iter_modules()` and calls `importlib.util.find_spec()`. If any module in `nodes/` has a broken import, this would raise during startup. | Low | Medium | The auto-import loop already guards with `if spec is None: continue` and only imports `.py` files directly under `nodes/` (not `arch/`). At this phase, no concrete node files exist, so the loop body never executes. The guard is sufficient. |
| The return-type annotation change from `-> list` to `-> list[dict]` could cause a type-checking warning if a linter is configured. However, the project does not appear to run mypy or pyright on worker code (per ENVIRONMENT.md §6, only `py_compile` is used for Python static checking). | Low | Low | `py_compile` does not check type annotations; it only verifies syntax. No lint tool is configured for Python code per the build commands. |
| Existing tests that assert `node_types == []` could fail if the dict-building logic produces non-empty output. | Very Low | Medium | `NODE_REGISTRY` is empty at this phase (no concrete node files exist), so the loop produces `[]`. The `test_nodes_init.py` tests confirm this invariant. |

## Acceptance Criteria

- [ ] `worker/.venv/bin/python -m py_compile worker/worker_main.py` exits 0
- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py -v -m "not real_mode"` exits 0
- [ ] `worker/.venv/bin/python -m pytest worker/tests/test_worker_main.py -v -m real_mode` exits 0
- [ ] `worker/.venv/bin/python -m pytest worker/tests/test_nodes_init.py -v` exits 0
