# Implementation Report: P24-D1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P24-D1                          |
| Phase         | 24 — Generic Conditioning/Sampling/Decode Nodes, Real Mode |
| Description   | worker/nodes/image.py: SaveImage node, mock branch only |
| Implemented   | 2026-07-19T15:00:00Z            |
| Status        | COMPLETE                        |

## Summary

Created `worker/nodes/image.py` with the `SaveImage` node class implementing its mock branch per `ANVILML_DESIGN.md §10.3` and `§14.6`. The mock branch generates a 64×64 black PNG via PIL, emits an `ImageReady` event dict via `ctx.emit`, and returns a sentinel dict. The real branch is a `NotImplementedError` placeholder deferred to P24-D2. Created `worker/tests/test_nodes_image.py` with 3 tests confirming mock emission, registry presence, and input validation. Updated `docs/TESTS.md` with entries for all 3 tests.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source         |
|--------|---------|------------------|----------------|
| python | Pillow  | 12.3.0           | venv already installed |

Pillow 12.3.0 was already installed in the worker venv. No new dependencies were introduced.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/image.py` | SaveImage node class with mock branch + placeholder real branch |
| CREATE | `worker/tests/test_nodes_image.py` | Tests for SaveImage: mock emission, registry, input validation |
| MODIFY | `docs/TESTS.md` | Added 3 entries for new tests |

## Commit Log

```
 .forge/reports/P24-D1_plan.md    | 171 +++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md     |   6 +-
 .forge/state/state.json          |  13 +--
 docs/TESTS.md                    |  32 ++++++++
 worker/nodes/image.py            | 117 +++++++++++++++++++++++++++
 worker/tests/test_nodes_image.py | 137 +++++++++++++++++++++++++++++++
 6 files changed, 467 insertions(+), 9 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: /home/dryw/AnvilML/worker/.pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 3 items

worker/tests/test_nodes_image.py::test_save_image_mock_emits_image_ready PASSED [ 33%]
worker/tests/test_nodes_image.py::test_save_image_in_registry PASSED     [ 66%]
worker/tests/test_nodes_image.py::test_save_image_missing_image_input_raises PASSED [100%]

============================== 3 passed in 0.11s ===============================
```

## Format Gate

```
(No output — cargo fmt --all -- --check exited 0)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 29.41s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 53.55s

# 3. Real-hardware Linux
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.52s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.52s
```

All four checks exit 0.

## Project Gates

### Gate 3 — Node Parity
`worker/tests/test_parity.py` does not exist yet (not triggered for this task — Gate 3 triggers on tasks that modify `crates/anvilml-core/src/node_registry.rs`).

### Gate 4 — Mock/Real Parity Markers
Both `grep -L` commands returned empty — all node files defining `execute()` have both `REAL_PATH_VERIFIED:` and `MOCK_PATH_VERIFIED:` markers.

### Mock/Real Parity Marker Verification
```
# MOCK_PATH_VERIFIED test collectible:
tests/test_nodes_image.py::test_save_image_mock_emits_image_ready
1 test collected in 0.02s
```

## Public API Delta

```
# New public items in worker/nodes/image.py:
17:class SaveImage(BaseNode):
52:    def execute(self, ctx: NodeContext, **inputs) -> dict:
```

New `pub` items:
- `class SaveImage(BaseNode)` — module path: `worker.nodes.image`
- `SaveImage.execute(self, ctx: NodeContext, **inputs) -> dict` — module path: `worker.nodes.image`

Both match the plan's Public API Surface table.

## Deviations from Plan

- **Input validation added to mock branch**: The approved plan's mock branch did not explicitly access `inputs["image"]`, but the plan's Test 3 (`test_save_image_missing_image_input_raises`) requires a `KeyError` when `image` is missing. I added `_= inputs["image"]` to the mock branch to validate the required input exists before proceeding. This ensures the test passes and follows the pattern of other nodes that validate required inputs. This is consistent with how `LoadModel` accesses `inputs["model_id"]` directly in its mock branch.
- **`defers_to: P24-D2` comment marker**: Added to the `execute()` method docstring and the `REAL_PATH_VERIFIED` marker comment block, per `FORGE_AGENT_RULES.md §9.7` and the task's `defers_to` field.
- **`REAL_PATH_VERIFIED` placeholder**: The approved plan named `worker/tests/test_nodes_image.py::test_save_image_real_emits_png` as the placeholder test. This test does not exist yet (it will be written by P24-D2). The marker convention (ANVILML_DESIGN.md §10.6) requires both markers to be present even when one test is not yet written — the marker names the future test that will satisfy the real path.

## Blockers

None.
