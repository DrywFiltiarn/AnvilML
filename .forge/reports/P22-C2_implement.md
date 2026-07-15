# Implementation Report: P22-C2

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P22-C2                          |
| Phase         | 22 — Qwen3 CLIP Arch Module     |
| Description   | worker/nodes/arch/clip/qwen3.py: key remap, load_state_dict, .arch attribute |
| Implemented   | 2026-07-16T01:45:00Z            |
| Status        | COMPLETE                        |

## Summary

Completed `qwen3.py`'s `load()` function by implementing all four steps of the loading contract (ANVILML_DESIGN.md §11.3): added `_build_key_remapping()` for checkpoint-key → module-key remapping, materialized the meta-constructed model via `to_empty()`, built the remapping table, cast tensors to target dtype BEFORE `load_state_dict(assign=True)`, and verified the `.arch` attribute persists after materialization. Added 6 new tests (2 unit tests for `_build_key_remapping()`, 4 integration tests for weight loading). Updated parity markers on `load()` to point at the new weight-verification tests. All 194 tests pass (117 mock + 77 real).

## Resolved Dependencies

None. This task uses only existing dependencies: `torch` (for `nn.Module.to_empty()`, `load_state_dict()`), `safetensors` (for `load_file()`), and `transformers` (for `AutoTokenizer`).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/clip/qwen3.py` | Add `_build_key_remapping()` function; update `load()` with materialization, key remapping, weight loading, and `.arch` safety net; update module docstring and `load()` docstring; update parity markers; add `load_file` import |
| MODIFY | `worker/tests/test_arch_clip_qwen3.py` | Add 6 new tests; update old test docstrings to reflect weight-loading behavior; update import to include `_build_key_remapping` |
| MODIFY | `docs/TESTS.md` | Add 6 new test entries for the new tests |

## Commit Log

 docs/TESTS.md                        |  70 +++++++++++
 worker/nodes/arch/clip/qwen3.py      | 205 ++++++++++++++++++++++++++++----
 worker/tests/test_arch_clip_qwen3.py | 219 ++++++++++++++++++++++++++++++++---
 3 files changed, 461 insertions(+), 33 deletions(-)

## Test Results

### Rust tests (all)
cargo test --workspace --features mock-hardware
All 360+ Rust tests passed with 0 failures.

### Python mock-mode tests
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v -m "not real_mode"
117 passed, 77 deselected in 5.96s

### Python real-mode tests
worker/.venv/bin/python -m pytest worker/tests/ -v -m real_mode
77 passed in 17.44s

### Full Python test suite
worker/.venv/bin/python -m pytest worker/tests/ -v
194 passed in 17.44s

### Specific new tests
- `test_build_key_remapping_direct_match`: PASSED
- `test_build_key_remapping_attention_remap`: PASSED
- `test_load_real_qwen3_fixture_with_weights`: PASSED
- `test_load_mock_qwen3_fixture_with_weights`: PASSED
- `test_load_weights_dtype_matches_target`: PASSED
- `test_load_arch_attribute_persists_after_materialization`: PASSED

## Format Gate

cargo fmt --all -- --check
(Exit 0 — no output means clean)

## Platform Cross-Check

### Check 1: Mock-hardware Linux
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 8.03s

### Check 2: Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 58.46s

### Check 3: Real-hardware Linux
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 01s

### Check 4: Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 04s

All four checks exit 0.

## Project Gates

### Gate 1 — Config Surface Sync
cargo test -p anvilml --features mock-hardware -- config_reference
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

### Gate 4 — Mock/Real Parity Markers
Markers verified:
- `worker/nodes/arch/clip/qwen3.py` has `REAL_PATH_VERIFIED` pointing to `test_load_real_qwen3_fixture_with_weights`
- `worker/nodes/arch/clip/qwen3.py` has `MOCK_PATH_VERIFIED` pointing to `test_load_mock_qwen3_fixture_with_weights`
Both named tests are collectible via `pytest --collect-only`.

## Public API Delta

No new pub items introduced. `_build_key_remapping` is private (underscore-prefixed). The `load()` function signature is unchanged.

## Deviations from Plan

1. **`_build_key_remapping()` implementation differs from the plan's draft.** The plan's pseudocode checked q, k, v against the same key in a single iteration (which is logically impossible — a single key cannot match three different patterns simultaneously). The actual implementation collects q/k/v keys by layer prefix first, then remaps them together when all three are present. This is the correct algorithm for the Qwen3 attention remapping pattern. The plan's approach would have produced empty remapping for all attention keys.

2. **Updated `test_load_real_qwen3_fixture` and `test_load_mock_qwen3_fixture` to match new behavior.** These tests previously asserted meta-device params, but `load()` now materializes weights. Updated them to assert CPU device (with weights loaded) to match the new behavior. This is a necessary consequence of the implementation — the old assertions were no longer valid.

3. **Updated `test_dtype_selection_bf16_real` docstring.** Changed the comment from "meta device, but dtype metadata is set via model.to(target_dtype) before materialization" to "loaded onto CPU, dtype set via model.to(target_dtype) before materialization and tensor casting" to accurately describe the new behavior.

## Blockers

None.
