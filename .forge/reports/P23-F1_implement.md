# Implementation Report: P23-F1

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P23-F1                          |
| Phase         | 023 — ZiT VAE Arch Module       |
| Description   | Runnable Proof: full load+sample+decode chain produces a real PIL image |
| Implemented   | 2026-07-17T17:10:00Z            |
| Status        | COMPLETE                        |

## Summary

Created `worker/tests/test_e2e_zit_pipeline.py` with three real-mode integration tests that chain the ZiT diffusion model loading (`zit.load()`), Sampler node execution (`Sampler.execute()`), and VAE decoding (`decode()`) end-to-end against fixture checkpoints. All three tests pass, producing real `PIL.Image.Image` objects with correct dimensions (8×8 RGB). Updated `docs/TESTS.md` with catalogue entries for all three new tests.

## Resolved Dependencies

| Type   | Name  | Version resolved | Source         |
|--------|-------|------------------|----------------|
| python | torch | (project venv)   | N/A — existing |
| python | pillow| (project venv)   | N/A — existing |

No new dependencies introduced. The task uses only existing packages already available in the worker venv.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/tests/test_e2e_zit_pipeline.py` | New e2e integration test: LoadModel → Sampler → decode() chain |
| MODIFY | `docs/TESTS.md` | Add test catalogue entries for the 3 new e2e tests |
| MODIFY | `.forge/state/CURRENT_TASK.md` | Task state update (inherited modification) |
| MODIFY | `.forge/state/state.json` | Task state update (inherited modification) |

## Commit Log

```
 .forge/state/CURRENT_TASK.md |  6 +++---
 .forge/state/state.json      | 13 +++++++------
 docs/TESTS.md                | 30 ++++++++++++++++++++++++++++++
 worker/tests/test_e2e_zit_pipeline.py | 219 ++++++++++++++++++++++++++++++++++
 4 files changed, 43 insertions(+), 9 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.1, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker
configfile: pyproject.toml
plugins: anyio-4.14.1
collecting ... collected 3 items

worker/tests/test_e2e_zit_pipeline.py::test_e2e_full_chain_produces_pil_image PASSED [ 33%]
worker/tests/test_e2e_zit_pipeline.py::test_e2e_batch_produces_multiple_images PASSED [ 66%]
worker/tests/test_e2e_zit_pipeline.py::test_e2e_image_is_real_pil_not_mock PASSED [100%]

=============================== warnings summary ===============================
tests/test_e2e_zit_pipeline.py::test_e2e_full_chain_produces_pil_image
tests/test_e2e_zit_pipeline.py::test_e2e_batch_produces_multiple_images
tests/test_e2e_zit_pipeline.py::test_e2e_image_is_real_pil_not_mock
  /home/dryw/AnvilML/worker/nodes/arch/vae/zit_vae.py:900: RuntimeWarning: invalid value encountered in cast
    decoded_np = (decoded_np * 255).astype("uint8")

========================= 3 passed, 3 warnings in 4.61s =========================
```

All 98 real-mode tests pass, all 127 mock-mode tests pass, all 367 Rust tests pass.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.90s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 55.97s

# 3. Real-hardware Linux
cargo check --bin anvilml
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.51s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.76s
```

All four cross-checks pass.

## Project Gates

### Gate 1 — Config Surface Sync
```
cargo test -p anvilml --features mock-hardware -- config_reference
    Running tests/config_reference.rs
    running 1 test
    test tests::config_reference_matches_defaults ... ok
    test result: ok. 1 passed; 0 failed
```

### Gate 2 — OpenAPI Drift
Not triggered — this task does not modify handler signatures, utoipa annotations, or AppState fields.

### Gate 3 — Node Parity
Not triggered — this task does not add, remove, or rename node types.

### Gate 4 — Mock/Real Parity Markers
Not triggered — this task does not add or modify node execute() or arch module load()/sample()/decode()/compute_latent_shape() functions. The e2e tests exercise existing functions through their real path.

## Public API Delta

```
(git diff HEAD -- worker/tests/test_e2e_zit_pipeline.py docs/TESTS.md | grep '^+.*pub ' | head -40)
(no output)
```

No new pub items introduced. This task only creates a test file — no public API changes.

## Deviations from Plan

- The plan called for `Sampler.execute(ctx, model=model, ...)` but `execute()` is an instance method (not a class method), so I used `node = Sampler(); result = node.execute(ctx, model=model, ...)` to match the established pattern in `test_nodes_sampler.py`. This is a minor API usage correction, not a deviation from the plan's intent.

## Blockers

None.
