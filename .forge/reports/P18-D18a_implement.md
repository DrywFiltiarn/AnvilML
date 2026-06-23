# Implementation Report: P18-D18a

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P18-D18a                                    |
| Phase       | 018 — ZiT Generic Nodes                     |
| Description | worker/nodes/arch/diffusion/zit.py: assemble ZImagePipeline from cached components |
| Implemented | 2026-06-23T13:30:00Z                        |
| Status      | COMPLETE                                    |

## Summary

Replaced the `NotImplementedError` stub in `sample()`'s real mode path with pipeline assembly logic that constructs a `ZImagePipeline` from cached components via `pipeline_cache.get_or_load()`. Added `vae` and `pipeline_cache` parameters to `sample()`. Fixed the module docstring to correctly describe the `diffusers` callback shape (`callback_on_step_end(self, i, t, callback_kwargs)`) instead of the wrong `emit_progress(step, total)`. Updated the test file to verify `get_or_load` is called with the correct pipeline cache key. Added `# defers_to: P18-D18c` comment at the stub site per FORGE_AGENT_RULES.md §9.7.

## Resolved Dependencies

| Type   | Name                  | Version resolved | Source         |
|--------|-----------------------|------------------|----------------|
| python | diffusers             | 0.38.0           | pypi-query MCP |

Verified `ZImagePipeline` and `FlowMatchEulerDiscreteScheduler` exist at diffusers 0.38.0. The project's `worker/requirements/base.txt` requires `diffusers>=0.38.0`, which is the floor version.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/diffusion/zit.py` | Replace NotImplementedError with pipeline assembly; fix module docstring; add `vae` and `pipeline_cache` parameters to `sample()` |
| MODIFY | `worker/tests/test_arch_zit.py` | Replace `test_sample_real_path_raises_not_implemented` with `test_sample_real_assembles_pipeline_via_cache` |
| MODIFY | `docs/TESTS.md` | Add entry for `test_sample_real_assembles_pipeline_via_cache` |

## Commit Log

```
 .forge/reports/P18-D18a_plan.md    | 130 +++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md       |   6 +-
 .forge/state/state.json            |  13 ++--
 docs/TESTS.md                      |   9 +++
 worker/nodes/arch/diffusion/zit.py | 108 +++++++++++++++++++++++-------
 worker/tests/test_arch_zit.py      |  55 +++++++++++++---
 6 files changed, 277 insertions(+), 44 deletions(-)
```

## Test Results

```
worker/tests/test_arch_zit.py:
  test_vae_scale_factor_value PASSED
  test_can_handle_zit PASSED
  test_can_handle_non_zit PASSED
  test_sample_mock_returns_mock_latent_and_seed PASSED
  test_sample_mock_preserves_seed_value PASSED
  test_sample_real_assembles_pipeline_via_cache PASSED
  test_sample_mock_no_torch_import PASSED
  test_compute_latent_shape_known_dims PASSED
  test_compute_latent_shape_non_divisible PASSED
  9 passed in 0.80s

Full worker test suite: 87 passed in 2.76s
Full Rust test suite: all crates pass (200+ tests)
```

## Format Gate

```
cargo fmt --all -- --check
# exit 0 — no changes needed
```

## Platform Cross-Check

Not required — this task only modifies Python files that run in the worker subprocess. The Rust platform cross-checks (Linux mock, Windows mock, Linux real, Windows real) are unaffected since no Rust source was modified.

## Project Gates

### Gate 1 — Config Surface Sync
```
cargo test -p anvilml --features mock-hardware -- config_reference
# config_reference ... ok
# 1 passed; 0 failed
```

### Gate 2 — OpenAPI Drift
```
cargo run -p anvilml-openapi && git diff --exit-code api/openapi.json
# No diff — openapi.json is current
```

### Gate 3 — Node Parity
Not triggered — no node types were added, removed, or renamed.

## Public API Delta

The `sample()` function signature changed with two new parameters:
- `vae: Any = None` — positional parameter (before keyword-only boundary)
- `pipeline_cache: Any = None` — keyword-only parameter (after `*`)

No new `pub` items introduced. No existing `pub` items removed.

## Deviations from Plan

1. **Added `vae` parameter to `sample()`**: The plan's `loader_fn` closure references `vae = vae  # passed as argument to sample()`, but the existing `sample()` signature had no `vae` parameter. I added `vae: Any = None` as a positional parameter before the keyword-only `pipeline_cache` to match the plan's intent. This is a non-breaking change since existing callers use keyword arguments and the default `None` handles mock-mode tests.

2. **Test name preserved**: The plan called for a new test `test_sample_real_assembles_pipeline_via_cache`. I replaced the existing `test_sample_real_path_raises_not_implemented` with this new test (same file location) rather than adding a second test, since the old test's assertion (NotImplementedError) is no longer valid.

3. **NotImplementedError still raised after get_or_load**: Per the plan, the pipeline is assembled but NOT invoked. The `NotImplementedError` is raised after `pipeline_cache.get_or_load()` returns, with the message "pipeline assembled but not yet invoked". The test catches this exception and verifies `get_or_load` was called before the exception was raised.

4. **`from unittest.mock import MagicMock` inside test body**: Moved the import inside the test function body (rather than at module top) to keep the test file's top-level imports clean and consistent with the pattern used in other tests.

## Blockers

None.
