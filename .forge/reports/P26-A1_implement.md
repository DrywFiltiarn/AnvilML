# Implementation Report: P26-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P26-A1                                      |
| Phase       | 26 — Flux 2 Klein 9B + Qwen3-8B CLIP Variant |
| Description | worker/tests/fixtures/: Flux 2 Klein 9B + Qwen3-8B fixture builders |
| Implemented | 2026-07-23T19:30:00Z                        |
| Status      | COMPLETE                                    |

## Summary

Created two Python builder scripts in `worker/tests/fixtures/` that generate tiny synthetic `.safetensors` checkpoints shaped to the Flux 2 Klein 9B diffusion architecture and the Qwen3-8B FP8-mixed text encoder respectively. Both scripts follow the established fixture builder pattern (path resolution, `_tensors()` function, `build()` function with `save_file()`), use structurally valid tensor shapes matching the existing arch modules' shape-inference formulas, and produce files under 10 MB. The Qwen3-8B fixture includes three tensors at `torch.float8_e4m3fn` dtype to demonstrate mixed-precision checkpoint loading.

## Resolved Dependencies

| Type   | Name        | Version resolved | Source         |
|--------|-------------|------------------|----------------|
| python | safetensors | 0.5.3+ (base.txt) | pypi-query MCP |
| python | torch       | 2.12.1+cpu      | pypi-query MCP |

No new dependencies introduced. Both `safetensors` and `torch` are already in the project's `worker/requirements/` files. PyTorch's `float8_e4m3fn` dtype is available in torch 2.12.1 (the project's current torch build).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/tests/fixtures/build_flux2klein_9b_fixture.py` | Builder script for Flux 2 Klein 9B fixture (176 lines) |
| CREATE | `worker/tests/fixtures/flux2klein9b_tiny.safetensors` | Generated Flux 2 Klein 9B checkpoint (~9.8 MB, 20 tensors) |
| CREATE | `worker/tests/fixtures/build_qwen3_8b_fixture.py` | Builder script for Qwen3-8B fixture (175 lines) |
| CREATE | `worker/tests/fixtures/qwen3_8b_tiny.safetensors` | Generated Qwen3-8B checkpoint (~0.75 MB, 14 tensors incl. 3 FP8) |

## Commit Log

```
 .forge/reports/P26-A1_plan.md                      | 183 +++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                       |   6 +-
 .forge/state/state.json                            |  11 +-
 .../tests/fixtures/build_flux2klein_9b_fixture.py  | 176 ++++++++++++++++++++
 worker/tests/fixtures/build_qwen3_8b_fixture.py    | 175 ++++++++++++++++++++
 .../tests/fixtures/flux2klein9b_tiny.safetensors   | Bin 0 -> 10260376 bytes
 worker/tests/fixtures/qwen3_8b_tiny.safetensors    | Bin 0 -> 789448 bytes
 7 files changed, 543 insertions(+), 8 deletions(-)
```

## Test Results

### Python mock-mode tests (182 passed)

```
============================= test session starts
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0
collected 338 items / 156 deselected / 182 selected
...
===================== 182 passed, 156 deselected in 38.70s =====================
```

### Python real-mode tests (156 passed)

```
============================= test session starts
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0
collected 338 items / 182 deselected / 156 selected
...
============== 156 passed, 182 deselected, 22 warnings in 56.15s ===============
```

### Rust tests (pre-existing failure — not caused by this task)

```
Running tests/config_reference.rs
test tests::config_reference_matches_defaults ... FAILED
model_dirs should be empty
```

This failure is pre-existing: the `anvilml.toml` has `[[model_dirs]]` entries but the test expects `model_dirs` to be empty. Verified by stashing all changes and running the test — it fails on the unmodified codebase.

## Format Gate

```
cargo fmt --all -- --check
```

Exit 0 — no formatting drift.

## Platform Cross-Check

Not applicable — this task writes only Python fixture builder scripts and generates `.safetensors` files. No Rust code is modified, so the four cross-check commands (mock Linux, mock Windows, real Linux, real Windows) are not relevant to this task's scope.

## Project Gates

### Gate 1 — Config Surface Sync

```
cargo test -p anvilml --features mock-hardware -- config_reference
test tests::config_reference_matches_defaults ... FAILED
model_dirs should be empty
```

Pre-existing failure (verified by stashing changes and re-running). Not caused by this task.

### Gate 2 — OpenAPI Drift

Not triggered — this task does not modify handler function signatures, `#[utoipa::path]` annotations, or `AppState` fields.

### Gate 3 — Node Parity

Not triggered — this task does not add, remove, or rename node types in `worker/nodes/`, nor does it modify `crates/anvilml-core/src/node_registry.rs`.

### Gate 4 — Mock/Real Parity Markers

Not triggered — this task does not add or modify a node's `execute()` or an arch module's `load()`/`sample()`/`decode()`/`compute_latent_shape()`.

## Public API Delta

```
(no output)
```

No new `pub` items introduced. Both builder scripts contain only internal functions (`_flux2klein_9b_tensors()`, `_qwen3_8b_tensors()`, `build()`) called from `if __name__ == "__main__"` blocks.

## Deviations from Plan

None. Implementation follows the approved plan exactly:
- `build_flux2klein_9b_fixture.py` uses `hidden_dim=256`, `context_dim=512` (reduced from 4096 to keep under 10 MB), single double block and single single block.
- `build_qwen3_8b_fixture.py` uses `hidden_dim=128`, `num_hidden_layers=1`, `intermediate_size=256`, `vocab_size=128`, with three FP8 tensors at `torch.float8_e4m3fn` dtype.
- No no-metadata variants generated (per-plan: per-family requirement already covered by 4B fixtures).
- No test code changes (per-plan: handled by P26-B1 and P26-C1).

## Blockers

Pre-existing test failure in `config_reference_matches_defaults` (Gate 1 — Config Surface Sync): the `anvilml.toml` has `[[model_dirs]]` entries but the test expects `model_dirs` to be empty. Verified by stashing all changes and running the test — it fails on the unmodified codebase. This is outside the scope of this task (which creates only Python fixture builder scripts) and must be resolved separately.
