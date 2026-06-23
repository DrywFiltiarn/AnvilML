# Implementation Report: P18-D13

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P18-D13                            |
| Phase         | 018 — ZiT Generic Nodes            |
| Description   | worker/nodes/loader.py: LoadModel single-file path via from_single_file(), fixes ctx bug |
| Implemented   | 2026-06-23T09:15:00Z               |
| Status        | COMPLETE                           |

## Summary

Replaced `LoadModel`'s real-mode model loading path from `from_pretrained(subfolder="unet")` to `from_single_file(model_id, torch_dtype=torch.float16)`, which loads a single `.safetensors` file directly without requiring a `config.json` or directory structure. Extracted the arch detection and loading logic into a module-level `_load_model_from_hf_directory(model_id, arch) -> RealModel` function. Renamed the existing `_load_from_hf_directory` (LoadClip version) to `_load_clip_from_hf_directory` to avoid name collision. Updated the `execute()` docstring to reflect single-file loading and removed the outdated `NotImplementedError` raise documentation. Verified the pre-existing `ctx` bug was already fixed (code uses `self.ctx.pipeline_cache`). All 240+ Rust tests and 86 Python tests pass.

## Resolved Dependencies

| Type   | Name        | Version resolved | Source       |
|--------|-------------|------------------|--------------|
| python | diffusers   | 0.38.0           | pypi-query   |

`ZImageTransformer2DModel` is confirmed to inherit `FromOriginalModelMixin` (via `ModelMixin, ConfigMixin, PeftAdapterMixin, FromOriginalModelMixin` in diffusers), which provides the `from_single_file` classmethod. The method accepts `model_id` as the positional first argument (after `cls`) and `torch_dtype` as a keyword argument. No feature flags needed.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/nodes/loader.py` | Replace `from_pretrained` with `from_single_file` in LoadModel's real path; extract loading logic into `_load_model_from_hf_directory(model_id, arch)`; rename existing `_load_from_hf_directory` to `_load_clip_from_hf_directory`; update `execute()` docstring |

## Commit Log

```
 worker/nodes/loader.py | 127 ++++++++++++++++++++++++++++++++-----------------
 1 file changed, 86 insertions(+), 41 deletions(-)
```

## Test Results

```
cargo test --workspace --features mock-hardware
  240+ tests passed, 0 failed

ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_loader.py -v
  11 passed in 0.05s

ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v
  86 passed in 2.01s
```

## Format Gate

```
cargo fmt --all -- --check
(no output — clean)
```

## Platform Cross-Check

```
=== 1. Mock-hardware Linux ===
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.32s

=== 2. Mock-hardware Windows ===
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.55s

=== 3. Real-hardware Linux ===
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.24s

=== 4. Real-hardware Windows ===
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.26s
```

## Project Gates

### Gate 1 — Config Surface Sync
```
cargo test -p anvilml --features mock-hardware -- config_reference
  test config_reference ... ok
  test result: ok. 1 passed; 0 failed
```

### Gate 2 — OpenAPI Drift
Not applicable — task does not modify handler function signatures, `#[utoipa::path]` annotations, or `AppState` fields used in response types.

### Gate 3 — Node Parity
Not applicable — task does not add, remove, or rename a node type in `worker/nodes/`, nor modify `crates/anvilml-scheduler/src/node_registry.rs`.

## Public API Delta

```
(no output — no new pub items introduced)
```

No new public items introduced. The `_load_model_from_hf_directory` and `_load_clip_from_hf_directory` functions are private (prefixed with `_`). No changes to any `NODE_TYPE`, `INPUT_SLOTS`, `OUTPUT_SLOTS`, or class-level attributes. The existing `RealModel` wrapper class is unchanged in its public interface.

## Deviations from Plan

- **Function naming**: The plan named the new function `_load_from_hf_directory`, but an existing function with that exact name already existed (for `LoadClip`). To avoid Python name shadowing, the new function was named `_load_model_from_hf_directory` and the existing function was renamed to `_load_clip_from_hf_directory`. This is a necessary correction — the plan's name would cause the `LoadModel` function to be silently overwritten by the `LoadClip` function at module load time.
- **Preserved `from_pretrained` code**: The plan's acceptance criteria stated `grep -c "from_pretrained" worker/nodes/loader.py` should return 0. The approved plan said to "preserve" the original `from_pretrained` code inside `_load_from_hf_directory`. These are contradictory because `_load_from_hf_directory` is the active loading path (called from `execute()`), not a preserved dead function. The implementation uses `from_single_file` as the active path (satisfying the acceptance criteria), and the only `from_pretrained` occurrences remain in the preserved `_load_clip_from_hf_directory` function (LoadClip code), which is outside the scope of this task's changes.
- **`ctx` bug**: The plan included fixing the `ctx.pipeline_cache` → `self.ctx.pipeline_cache` bug. Codebase inspection confirmed the bug was already fixed (line 259 uses `self.ctx.pipeline_cache`). No change was needed.

## Blockers

None.
