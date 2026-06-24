# Implementation Report: P904-A8

| Field         | Value                                                     |
|---------------|-----------------------------------------------------------|
| Task ID       | P904-A8                                                   |
| Phase         | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects)    |
| Description   | worker/nodes/loader.py: LoadModel and LoadVae real paths never move loaded components to ctx.device |
| Implemented   | 2026-06-24T10:15:00Z                                      |
| Status        | COMPLETE                                                  |

## Summary

Fixed the device-placement defect in `LoadModel` and `LoadVae` real-mode loading paths so that loaded transformer and VAE components are moved to `self.ctx.device` instead of silently defaulting to CPU. Added a `device: str = "cpu"` parameter to `_load_model_from_hf_directory()`, updated `LoadModel.execute()` to pass `self.ctx.device`, and modified `LoadVae.execute()`'s `loader_fn` closure to capture `self.ctx.device` and call `.to(device)` on the `AutoencoderKL` result. Added a unit test verifying the new function signature.

## Resolved Dependencies

None. This task introduces no new dependencies — it only modifies existing import patterns and function signatures within `worker/nodes/loader.py`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/loader.py` | Added `device: str = "cpu"` parameter to `_load_model_from_hf_directory()`, added `transformer = transformer.to(device)` call, updated `LoadModel.execute()` to pass `self.ctx.device`, updated `LoadVae.execute()` to capture `self.ctx.device` and call `.to(device)` on VAE |
| Modify | `worker/tests/test_nodes_loader.py` | Added `test_loadmodel_hf_directory_accepts_device_param` test verifying the new `device` parameter exists with default `"cpu"` |

## Commit Log

```
 .forge/reports/P904-A8_plan.md    | 217 ++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md      |   6 +-
 .forge/state/state.json           |  13 +--
 worker/nodes/loader.py            |  37 ++++++-
 worker/tests/test_nodes_loader.py |  44 ++++++++
 5 files changed, 304 insertions(+), 13 deletions(-)
```

## Test Results

```
cargo test --workspace --features mock-hardware
  175 tests passed, 0 failed

ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v
  93 tests passed, 0 failed
  Including: test_loadmodel_hf_directory_accepts_device_param (new)
```

## Format Gate

```
cargo fmt --all -- --check
(exit 0 — no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux (already checked in cargo check above)
# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.60s

# 3. Real-hardware Linux
cargo check --bin anvilml
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s
```

## Project Gates

```
# Gate 1 — Config Surface Sync
cargo test -p anvilml --features mock-hardware -- config_reference
  test config_reference ... ok
  test result: ok. 1 passed; 0 failed

# Gate 2 — OpenAPI Drift
cargo run -p anvilml-openapi && git diff --exit-code api/openapi.json
  (no diff — openapi.json is up to date)
```

## Public API Delta

```
git diff HEAD -- worker/nodes/loader.py worker/tests/test_nodes_loader.py | grep '^+.*pub ' | head -40
(no output)
```

No new `pub` items introduced. The only signature change is widening `_load_model_from_hf_directory`'s parameter list by adding a defaulted `device: str = "cpu"` parameter, which does not constitute a new public API item (it is an internal helper function).

## Deviations from Plan

None. Implementation followed the approved plan exactly:
- Step 1: Added `device: str = "cpu"` parameter to `_load_model_from_hf_directory()` with `.to(device)` call.
- Step 2: Updated `LoadModel.execute()` lambda to pass `self.ctx.device`.
- Step 3: Updated `LoadVae.execute()` to capture `self.ctx.device` and call `.to(device)` on VAE result.
- Step 4: Added `test_loadmodel_hf_directory_accepts_device_param` test with `importorskip` guard.

## Blockers

None.
