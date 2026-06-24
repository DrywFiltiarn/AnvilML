# Implementation Report: P904-B3

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P904-B3                         |
| Phase         | 904 — P18 D16–D20 Retrofit      |
| Description   | worker/nodes/loader.py: switch arch detection from safetensors-metadata-only to key-prefix-based detection |
| Implemented   | 2026-06-24T16:05:00+02:00       |
| Status        | COMPLETE                        |

## Summary

Added key-prefix-based architecture detection as the primary signal in `_load_model_from_safetensors` in `worker/nodes/loader.py`, matching the ComfyUI pattern of inspecting raw checkpoint key prefixes before any stripping. The new `_detect_arch_from_keys()` helper function inspects raw state dict keys for architecture-specific prefixes (currently `model.diffusion_model.` → `"zit"`). The detection order is now: keys → metadata → arch param. A new test `test_loadmodel_key_prefix_detects_zit` verifies the detection works on a real safetensors file with ZiT-like keys and no metadata.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| (none)  | —         | —                | No new dependencies; task uses only `safetensors.torch.load_file` already available via existing imports. |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/loader.py` | Add `_detect_arch_from_keys()` helper; modify `_load_model_from_safetensors()` detection order (keys → metadata → arch param) |
| MODIFY | `worker/tests/test_nodes_loader.py` | Add `test_loadmodel_key_prefix_detects_zit` test |
| MODIFY | `docs/TESTS.md` | Add test catalogue entry for new test |

## Commit Log

```
 docs/TESTS.md                     |   9 ++++
 worker/nodes/loader.py            |  98 +++++++++++++++++++++++++++++++--------
 worker/tests/test_nodes_loader.py |  66 ++++++++++++++++++++++++++
 3 files changed, 153 insertions(+), 20 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.0, pluggy-1.6.0 -- /home/dryw/AnvilML/worker/.venv/bin/python
cachedir: .pytest_cache
rootdir: /home/dryw/AnvilML/worker/tests
configfile: pytest.ini
plugins: anyio-4.14.0
collecting ... collected 105 items

worker/tests/test_nodes_loader.py::test_loadmodel_registered_in_registry PASSED
worker/tests/test_nodes_loader.py::test_loadmodel_execute_returns_mock_model PASSED
worker/tests/test_nodes_loader.py::test_loadmodel_execute_missing_model_id_defaults_empty PASSED
worker/tests/test_nodes_loader.py::test_loadmodel_metadata_attributes PASSED
worker/tests/test_nodes_loader.py::test_loadvae_registered_in_registry PASSED
worker/tests/test_nodes_loader.py::test_loadvae_execute_returns_mock_vae PASSED
worker/tests/test_nodes_loader.py::test_loadvae_metadata_attributes PASSED
worker/tests/test_nodes_loader.py::test_loadclip_registered_in_registry PASSED
worker/tests/test_nodes_loader.py::test_loadclip_execute_returns_mock_clip_default_type PASSED
worker/tests/test_nodes_loader.py::test_loadclip_execute_returns_mock_clip_explicit_type PASSED
worker/tests/test_nodes_loader.py::test_loadclip_metadata_attributes PASSED
worker/tests/test_nodes_loader.py::test_loadmodel_safetensors_accepts_device_param PASSED
worker/tests/test_nodes_loader.py::test_loadmodel_key_prefix_detects_zit PASSED
...
============================== 105 passed in 17.56s ==============================
```

Rust tests: 173 passed, 0 failed (full `cargo test --workspace --features mock-hardware`).

## Format Gate

```
cargo fmt --all -- --check
# exited 0 — no output (clean)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.31s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.55s

# 3. Real-hardware Linux
cargo check --bin anvilml
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
# Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.25s
```

All four checks exited 0.

## Project Gates

**Gate 1 — Config Surface Sync:**
```
cargo test -p anvilml --features mock-hardware -- config_reference
# test config_reference ... ok
# test result: ok. 1 passed; 0 failed
```

## Public API Delta

```
git diff HEAD -- worker/nodes/loader.py worker/tests/test_nodes_loader.py | grep '^+.*def \|^\+.*pub ' | head -40
+def _detect_arch_from_keys(checkpoint: dict[str, Any]) -> str | None:
+def test_loadmodel_key_prefix_detects_zit(tmp_path: pytest.TempPath) -> None:
```

No new `pub` items introduced. `_detect_arch_from_keys` is module-private (underscore-prefixed, not in `__all__`). `test_loadmodel_key_prefix_detects_zit` is a test function.

## Deviations from Plan

- The plan's test code included `del torch` after `pytest.importorskip("torch")`, which caused an `UnboundLocalError` because Python treated `torch` as a local variable (since it was assigned to in the function) but deleted it before later references. Removed the `del torch` line — the `importorskip` call already satisfies the "torch is installed" precondition, and the `torch` object is needed later for tensor creation in the test. This is a minor deviation from the plan's test code that was necessary to make the test compile and run correctly.

## Blockers

None.
