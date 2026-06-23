# Implementation Report: P18-D17

| Field         | Value                                           |
|---------------|-------------------------------------------------|
| Task ID       | P18-D17                                         |
| Phase         | 018 — ZiT Generic Nodes                         |
| Description   | worker/nodes/sampler.py: EmptyLatent gains optional model input and real noise tensor path |
| Implemented   | 2026-06-23T12:00:00Z                            |
| Status        | COMPLETE                                        |

## Summary

Added an optional `model:MODEL` input slot to `EmptyLatent.INPUT_SLOTS` and implemented the real-mode code path in `execute()`. The real path dispatches to the architecture module via `arch.get_module(model)`, reads `num_channels_latents` from `model.in_channels`, calls `mod.compute_latent_shape()` (delegated to the arch module because different architectures use structurally different packing schemes), and returns a `torch.randn` noise tensor. Mock mode is unchanged — the new slot is optional and ignored. All 87 Python tests pass, all 150+ Rust tests pass, all gates pass, and all cross-checks pass.

## Resolved Dependencies

None. No new external dependencies are introduced. The task uses `torch` which is already a dependency of the Python worker (`worker/requirements/base.txt`), imported lazily inside the real-mode path.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/sampler.py` | Added `arch` import; added `SlotSpec("model", "MODEL", optional=True)` as 4th INPUT_SLOT; replaced `NotImplementedError` stub with real-mode code path (arch dispatch, shape computation, torch.randn); updated class docstring and execute docstring |
| MODIFY | `worker/tests/test_nodes_sampler.py` | Updated `test_emptylatent_metadata_attributes` to expect 4 INPUT_SLOTS and added assertion for the 4th slot (model, MODEL, optional=True) |
| MODIFY | `docs/TESTS.md` | Added 4 entries for EmptyLatent tests (modified metadata test + 3 existing tests that verify mock-mode behavior) |

## Commit Log

```
 .forge/reports/P18-D17_plan.md     | 116 +++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md       |   6 +-
 .forge/state/state.json            |  13 +++--
 docs/TESTS.md                      |  36 ++++++++++++
 worker/nodes/sampler.py            |  65 ++++++++++++++++-----
 worker/tests/test_nodes_sampler.py |  19 ++++--
 6 files changed, 227 insertions(+), 28 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.0, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
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

============================== 9 passed in 0.11s ===============================
```

Full Python suite: 87 passed in 2.01s.
Full Rust suite: all tests passed across all crates (anvilml, anvilml-core, anvilml-hardware, anvilml-ipc, anvilml-registry, anvilml-scheduler, anvilml-server, anvilml-worker, anvilml-artifacts).

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
cargo check --workspace --features mock-hardware            → Finished (0.31s)
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu → Finished (0.54s)
cargo check --bin anvilml                                   → Finished (0.23s)
cargo check --bin anvilml --target x86_64-pc-windows-gnu   → Finished (0.25s)
```

All four cross-checks passed with zero errors.

## Project Gates

**Gate 1 — Config Surface Sync:**
```
cargo test -p anvilml --features mock-hardware -- config_reference
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Gate 2 — OpenAPI Drift:**
```
cargo run -p anvilml-openapi && git diff --exit-code api/openapi.json
→ No diff (openapi.json is up to date)
```

**Gate 3 — Node Parity:**
```
Gate not applicable — test_parity.py does not exist yet in the repo.
```

## Public API Delta

```
$ git diff HEAD -- worker/nodes/sampler.py | grep '^+.*SlotSpec'
+        SlotSpec("model", "MODEL", optional=True),
```

The only new public item is the 4th element in `EmptyLatent.INPUT_SLOTS`:
- `EmptyLatent.INPUT_SLOTS[3]` → `SlotSpec("model", "MODEL", optional=True)`

No new `pub` functions, structs, traits, or enums were introduced. The `execute()` method signature is unchanged.

## Deviations from Plan

None. All changes were implemented exactly as specified in the approved plan:
- The optional `model` slot was added as the 4th slot (after `batch_size`)
- The real-mode path dispatches via `arch.get_module(model)`, reads `model.in_channels`, calls `mod.compute_latent_shape(batch_size, height, width, num_channels_latents)`, and returns `torch.randn(shape, dtype=torch.float32, device=ctx.device)`
- The docstring was updated to replace the `NotImplementedError` mention with a `ValueError` description
- The test was updated to expect 4 INPUT_SLOTS with the 4th slot assertion
- Inline comments were added explaining why shape computation is delegated to the arch module (architecture-specific packing schemes, e.g. Flux 2 Klein)

## Blockers

None.
