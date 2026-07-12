# Implementation Report: P19-C1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P19-C1                          |
| Phase         | 19 — Model Loading Contract Groundwork |
| Description   | worker/nodes/loader.py: LoadModel node, mock branch only |
| Implemented   | 2026-07-13T00:00:00Z            |
| Status        | COMPLETE                        |

## Summary

Created `worker/nodes/loader.py` with the `LoadModel` node class implementing the exact
slot specification from ANVILML_DESIGN.md §10.3: NODE_TYPE="LoadModel", CATEGORY="Loaders",
one string input (model_id), one model output (MODEL). The execute() method branches on
ctx.mock at the top — the mock branch returns a sentinel dict {"model": {"mock": True,
"model_id": inputs["model_id"]}}, and the real branch raises NotImplementedError
(deferred to P19-C2). The class is @register-decorated so it appears in NODE_REGISTRY.
Ship three tests verifying the mock sentinel shape, node registry registration, and
real-mode NotImplementedError.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| (none) | (none)    | (n/a)            | (n/a)          |

No new dependencies. This task uses only existing types from worker.nodes.base:
BaseNode, SlotSpec, NodeContext, and register.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | worker/nodes/loader.py | LoadModel node class with mock/real execute branches, @register decorated, dual-mode parity markers, defers_to marker |
| CREATE | worker/tests/test_nodes_loader.py | Tests: mock sentinel shape, registry presence, real-mode NotImplementedError |
| CREATE | docs/TESTS.md (appended) | 3 new test entries for the new tests |
| MODIFY | crates/anvilml-worker/tests/real_startup_tests.rs | Fixed brittle index-0 assertion on node_types order |
| MODIFY | worker/tests/test_worker_main.py | Fixed 3 brittle index-0 assertions on node_types order |

## Commit Log

 .forge/reports/P19-C1_plan.md                     | 178 ++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                      |   6 +-
 .forge/state/state.json                           |  13 +-
 crates/anvilml-worker/tests/real_startup_tests.rs |  12 +-
 docs/TESTS.md                                     |  36 +++++
 worker/nodes/loader.py                            |  74 +++++++++
 worker/tests/test_nodes_loader.py                 | 102 +++++++++++++
 worker/tests/test_worker_main.py                  |  35 ++++-
 8 files changed, 436 insertions(+), 20 deletions(-)

## Test Results

### Rust tests (cargo test --workspace --features mock-hardware)

All 443 Rust tests passed. The one previously-failing test
(test_real_subprocess_sends_ready) now passes after fixing the brittle
index-0 assertion to check for PassThrough presence anywhere in the
node_types list.

### Python mock-mode tests (ANVILML_WORKER_MOCK=1, -m "not real_mode")

88 passed, 23 deselected. All new tests included:
- worker/tests/test_nodes_loader.py::test_load_model_mock_returns_sentinel PASSED
- worker/tests/test_nodes_loader.py::test_load_model_in_registry PASSED

### Python real-mode tests (-m real_mode)

23 passed, 88 deselected. All new tests included:
- worker/tests/test_nodes_loader.py::test_load_model_real_raises_not_implemented PASSED

## Format Gate

```
cargo fmt --all -- --check
```

Exit 0, no output (no formatting drift).

## Platform Cross-Check

### 1. Mock-hardware Linux
```
cargo check --workspace --features mock-hardware
```
Exit 0. Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.10s.

### 2. Mock-hardware Windows
```
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
```
Exit 0. Finished `dev` profile [unoptimized + debuginfo] target(s) in 55.13s.

### 3. Real-hardware Linux
```
cargo check --bin anvilml
```
Exit 0. Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.39s.

### 4. Real-hardware Windows
```
cargo check --bin anvilml --target x86_64-pc-windows-gnu
```
Exit 0. Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.18s.

## Project Gates

### Gate 1 — Config Surface Sync
Not triggered. This task does not add, rename, or remove any field on ServerConfig.

### Gate 2 — OpenAPI Drift
Not triggered. This task does not modify handler function signatures, utoipa annotations,
or AppState fields.

### Gate 3 — Node Parity
Gate 3 test file `worker/tests/test_parity.py` does not exist yet. This is a pre-existing
gap in the project — the gate is defined in ENVIRONMENT.md §8 but the test has not been
created. This task adds a node type in `worker/nodes/` which would normally trigger Gate 3.

### Gate 4 — Mock/Real Parity Markers
Both checks pass:
1. `grep -rn "REAL_PATH_VERIFIED:\|MOCK_PATH_VERIFIED:" worker/nodes/` — all marker test
   IDs resolve to collectible tests:
   - `worker/tests/test_nodes_loader.py::test_load_model_real_raises_not_implemented` — 1 test collected
   - `worker/tests/test_nodes_loader.py::test_load_model_mock_returns_sentinel` — 1 test collected
   - `worker/tests/test_passthrough.py::test_execute_real_returns_input` — 1 test collected
   - `worker/tests/test_passthrough.py::test_execute_mock_returns_input` — 1 test collected
2. `grep -L "REAL_PATH_VERIFIED:"` and `grep -L "MOCK_PATH_VERIFIED:"` on all node .py
   files (excluding __init__.py, base.py, and MARKER_CONVENTION.md) return empty — every
   file defining execute() has both markers present.

## Public API Delta

No new `pub` items (Python does not use `pub`). The new public items are Python classes
and methods:

- `class LoadModel(BaseNode)` — `worker/nodes/loader.py`
  - `NODE_TYPE = "LoadModel"` (class attribute)
  - `CATEGORY = "Loaders"` (class attribute)
  - `DISPLAY_NAME = "Load Model"` (class attribute)
  - `DESCRIPTION = "Loads a diffusion model from a safetensors file."` (class attribute)
  - `INPUT_SLOTS = [SlotSpec("model_id", "STRING")]` (class attribute)
  - `OUTPUT_SLOTS = [SlotSpec("model", "MODEL")]` (class attribute)
  - `execute(self, ctx: NodeContext, **inputs) -> dict` (method)

## Deviations from Plan

- **Brittle assertion fixes in pre-existing tests:** Adding LoadModel to the node system
  changed the order of node types in NODE_REGISTRY (LoadModel sorts before PassThrough
  alphabetically in the auto-import loop). This caused three tests in
  `crates/anvilml-worker/tests/real_startup_tests.rs` and three tests in
  `worker/tests/test_worker_main.py` to fail because they asserted `node_types[0].type_name
  == "PassThrough"`. Fixed by changing to `any(nt.type_name == "PassThrough" for nt in
  node_types)` — checking presence anywhere in the list rather than at a fixed index. This
  is a minimal, correct fix that preserves the test's intent (verify PassThrough is
  registered) while making it robust to future node additions.

## Blockers

None.

## Gate 3 Note

The Node Parity gate (Gate 3) references `worker/tests/test_parity.py` which does not
exist in the codebase. This is a pre-existing gap — the gate is defined in ENVIRONMENT.md
§8 but the test file has not been created yet. This does not block this task; it is a
separate issue to be addressed when the test file is authored.
