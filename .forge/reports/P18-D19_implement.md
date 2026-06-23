# Implementation Report: P18-D19

| Field         | Value                                                         |
|---------------|-----------------------------------------------------------------|
| Task ID       | P18-D19                                                         |
| Phase         | 018 — ZiT Generic Nodes                                         |
| Description   | worker/nodes/sampler.py: Sampler real path dispatches to arch.get_module().sample() |
| Implemented   | 2026-06-23T15:10:00Z                                            |
| Status        | COMPLETE                                                        |

## Summary

Replaced the `NotImplementedError` stub in `Sampler.execute()` (lines 300–309) with actual architecture-module dispatch. The new real path builds an `emit_progress(step, total)` callable that wraps `self.ctx.emit` into a `Progress` event dict, calls `arch.get_module(model)` to resolve the matching architecture module, raises `ValueError("unsupported model architecture")` if no module claims the model, invokes `mod.sample()` with all required arguments, and returns `{"latent": result[0], "seed": result[1]}`. The docstring's `Raises` section was updated from `NotImplementedError` to `ValueError`. All 90 Python tests and all Rust tests pass.

## Resolved Dependencies

None. This task introduces no new dependencies — it only modifies existing code paths within `worker/nodes/sampler.py`, calling into already-imported modules (`worker.nodes.arch`, `worker.nodes.base`).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/sampler.py` | Replaced `Sampler.execute()`'s real-path stub with `arch.get_module()` dispatch to `mod.sample()`; updated docstring `Raises` section; added inline comments at dispatch site |

## Commit Log

```
 .forge/reports/P18-D19_plan.md  | 135 +++++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md    |   6 +-
 .forge/state/state.json         |  13 ++--
 worker/nodes/sampler.py         |  52 ++++++++++++----
 4 files changed, 185 insertions(+), 21 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.0, pluggy-1.6.0, cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
plugins: anyio-4.14.0
collecting ... collected 9 items

worker/tests/test_nodes_sampler.py::test_emptylatent_registered_in_registry PASSED [ 11%]
worker/tests/test_nodes_sampler.py::test_emptylatent_execute_returns_mock_latent PASSED [ 22%]
worker/tests/test_nodes_sampler.py::test_emptylatent_default_batch_size PASSED [ 33%]
worker/tests/test_nodes_sampler.py::test_sampler_registered_in_registry PASSED [ 44%]
worker/tests/test_nodes_sampler.py::test_sampler_execute_returns_mock_latent_and_seed PASSED [ 55%]
worker/tests/test_nodes_sampler.py::test_sampler_seed_negative_one_resolves_to_random PASSED [ 66%]
worker/tests/test_nodes_sampler.py::test_sampler_emits_progress_flag PASSED [ 77%]
worker/tests/test_nodes_sampler.py::test_sampler_metadata_attributes PASSED [ 88%]
worker/tests/test_nodes_sampler.py::test_emptylatent_metadata_attributes PASSED [100%]

============================== 9 passed in 0.05s ===============================
```

Full Python suite (90 tests): all passed in 3.05s.
Full Rust suite: all passed (200+ tests across all crates).

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.88s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.56s

# 3. Real-hardware Linux
cargo check --bin anvilml
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.30s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.29s
```

All four exit 0.

## Project Gates

**Gate 1 — Config Surface Sync:**
```
cargo test -p anvilml --features mock-hardware -- config_reference
  running 1 test
  test config_reference ... ok
  test result: ok. 1 passed; 0 failed
```

**Gate 2 — OpenAPI Drift:** Not triggered — task does not modify handler signatures, `#[utoipa::path]` annotations, or `AppState` fields.

**Gate 3 — Node Parity:** Not applicable — `worker/tests/test_parity.py` does not yet exist in the codebase.

## Public API Delta

```
(no output — no new pub items introduced)
```

No new `pub` items introduced. The only change is to the private implementation of `Sampler.execute()` (an `@abstractmethod` implementation from `BaseNode`). No function signatures changed.

## Deviations from Plan

- The plan's code snippet used `ctx.device` and `ctx.cancel_flag` as shorthand notation. In the actual codebase, the node context is accessed via `self.ctx` (set by `BaseNode.__init__`), so the implementation uses `self.ctx.device` and `self.ctx.cancel_flag`. This is consistent with the existing `EmptyLatent` node's pattern (which also accesses `self.ctx` attributes). Documented here for clarity.
- The `cancel_flag` in `NodeContext` is typed as `Any` and is a `list[bool]` in the test fixture. The `sample()` function in `zit.py` calls `cancel_flag.is_set()` (line 112), which expects a `threading.Event`. This known API shape mismatch is unchanged — the real path is unreachable in mock-mode tests, and the prerequisite task (P18-D18c) is expected to reconcile this. No action taken in this task.

## Blockers

None.
