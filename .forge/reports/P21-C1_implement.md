# Implementation Report: P21-C1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P21-C1                          |
| Phase         | 21 — ZiT Diffusion Arch Module: Sampling & Latent Shape |
| Description   | worker/nodes/sampler.py: Sampler generic node, mock branch only |
| Implemented   | 2026-07-14T18:00:00Z            |
| Status        | COMPLETE                        |

## Summary

Created the `Sampler` generic node in `worker/nodes/sampler.py` with all six required class attributes (NODE_TYPE, CATEGORY, DISPLAY_NAME, DESCRIPTION, INPUT_SLOTS with 7 SlotSpecs, OUTPUT_SLOTS with 2 SlotSpecs), a mock branch that returns a sentinel dict with propagated latent shape and deterministic seed resolution (-1 → 0), and a real branch that raises `NotImplementedError` deferred to P21-C2. The node is registered in `NODE_REGISTRY` via `@register`. Created `worker/tests/test_nodes_sampler.py` with 5 tests covering class attributes, mock shape propagation, mock seed resolution, real-mode NotImplementedError, and registry presence. Updated `docs/TESTS.md` with entries for all 5 tests.

## Resolved Dependencies

None. The Sampler node imports only from `worker.nodes.base` (BaseNode, NodeContext, SlotSpec, register), all of which already exist. No external crates or packages are introduced.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/nodes/sampler.py` | New file: `Sampler` node class with mock branch + stub real branch, dual-mode parity markers, `@register` decorator. |
| CREATE | `worker/tests/test_nodes_sampler.py` | New file: 5 tests for `Sampler` (class attributes, mock shape, mock seed, real raises, registry). |
| MODIFY | `docs/TESTS.md` | Added 5 entries for the new tests using ANVILML_DESIGN.md §17.1 format. |

## Commit Log

```
 worker/nodes/sampler.py          |  89 +++++++++++++++++++++++++++++++++++++++++
 worker/tests/test_nodes_sampler.py | 172 +++++++++++++++++++++++++++++++++++++++++++++++++++++
 docs/TESTS.md                    |  64 +++++++++++++++++++++++++
 3 files changed, 325 insertions(+)
```

## Test Results

```
============================= test session starts ==============================
platform linux 3.12.3, pytest-9.1.1, pluggy-1.6.0
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
collecting ... collected 5 items

worker/tests/test_nodes_sampler.py::test_sampler_class_attributes PASSED [ 20%]
worker/tests/test_nodes_sampler.py::test_sampler_mock_returns_expected_shape PASSED [ 40%]
worker/tests/test_nodes_sampler.py::test_sampler_mock_seed_zero PASSED   [ 60%]
worker/tests/test_nodes_sampler.py::test_sampler_real_raises_not_implemented PASSED [ 80%]
worker/tests/test_nodes_sampler.py::test_sampler_in_registry PASSED      [100%]

============================== 5 passed in 0.10s ===============================
```

Real-mode subset (1 test):
```
worker/tests/test_nodes_sampler.py::test_sampler_real_raises_not_implemented PASSED [100%]
======================= 1 passed, 4 deselected in 0.02s ========================
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.39s — CHECK1 OK

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.76s — CHECK2 OK

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.27s — CHECK3 OK

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.28s — CHECK4 OK
```

## Project Gates

### Gate 1 — Config Surface Sync
```
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed
```

### Gate 4 — Mock/Real Parity Markers
Both markers on `Sampler.execute()` are collectible:
```
tests/test_nodes_sampler.py::test_sampler_real_raises_not_implemented — 1 test collected
tests/test_nodes_sampler.py::test_sampler_mock_returns_expected_shape — 1 test collected
```

## Public API Delta

No new `pub` items introduced. The `Sampler` class is a regular Python class (no `pub` keyword) registered in `NODE_REGISTRY` via the `@register` decorator side-effect.

## Deviations from Plan

None. All implementation steps matched the approved plan exactly.

## Blockers

None.
