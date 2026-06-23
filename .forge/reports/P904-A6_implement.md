# Implementation Report: P904-A6

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P904-A6                                           |
| Phase         | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects) |
| Description   | worker/nodes/arch/diffusion/zit.py + sampler.py: loader_fn reads tokenizer/text_encoder off the wrong object |
| Implemented   | 2026-06-23T23:30:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Fixed the wiring defect in `zit.py`'s `loader_fn` which was reading `.tokenizer` and `.text_encoder` off the `conditioning` object instead of off the `clip` object. Added a `clip` parameter to `sample()`'s signature, wired `clip` through `Sampler.INPUT_SLOTS` and `Sampler.execute()`, and updated the two affected real-mode tests and one metadata test to pass a mock `clip` object. Updated `docs/TESTS.md` catalogue entries for both tests.

## Resolved Dependencies

None. This task only modifies existing Python source files and does not introduce any new dependencies.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/arch/diffusion/zit.py` | Added `clip` parameter to `sample()` signature; fixed `loader_fn` to read tokenizer/text_encoder from `clip` |
| MODIFY | `worker/nodes/sampler.py` | Added `SlotSpec("clip", "CLIP")` to `INPUT_SLOTS`; added `clip` read and pass-through to `mod.sample()` call |
| MODIFY | `worker/tests/test_arch_zit.py` | Updated two real-mode tests to pass mock `clip`; removed `.tokenizer`/`.text_encoder` from conditioning mocks |
| MODIFY | `worker/tests/test_nodes_sampler.py` | Updated `test_sampler_metadata_attributes` to expect 7 INPUT_SLOTS (was 6) and added `clip` slot spec check |
| MODIFY | `docs/TESTS.md` | Updated catalogue entries for both real-mode tests to reflect the `clip` argument |

## Commit Log

```
 .forge/state/CURRENT_TASK.md                   |  6 +++---
 .forge/state/state.json                        | 13 +++++++------
 docs/TESTS.md                                  | 12 ++++++------
 worker/nodes/arch/diffusion/zit.py             | 29 ++++++++++++++-----------
 worker/nodes/sampler.py                        | 20 ++++++++++++-------
 worker/tests/test_arch_zit.py                  | 18 ++++++++++++++---
 worker/tests/test_nodes_sampler.py             | 24 ++++++++++++++++--------
 7 files changed, 77 insertions(+), 45 deletions(-)
```

## Test Results

```
============================= test session starts ==============================
platform linux -- Python 3.12.3, pytest-9.1.0, pluggy-1.6.0, collected 92 items

worker/tests/test_arch_zit.py: 11 passed
worker/tests/test_nodes_sampler.py: 9 passed
worker/tests/test_arch_clip_init.py: 3 passed
worker/tests/test_arch_clip_l.py: 4 passed
worker/tests/test_arch_clip_qwen3.py: 4 passed
worker/tests/test_arch_clip_t5.py: 4 passed
worker/tests/test_arch_init.py: 3 passed
worker/tests/test_executor.py: 9 passed
worker/tests/test_ipc.py: 7 passed
worker/tests/test_nodes_base.py: 4 passed
worker/tests/test_nodes_decode.py: 5 passed
worker/tests/test_nodes_encoder.py: 5 passed
worker/tests/test_nodes_loader.py: 7 passed
worker/tests/test_pipeline_cache.py: 5 passed
worker/tests/test_placeholder.py: 1 passed
worker/tests/test_worker_main.py: 6 passed

============================= 92 passed in 16.90s ==============================
```

Rust tests (all passed):
- `cargo test --workspace --features mock-hardware`: 172 tests passed, 0 failed

## Format Gate

```
cargo fmt --all -- --check
```
Exit 0 — no formatting drift detected.

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.30s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.61s

# 3. Real-hardware Linux
cargo check --bin anvilml
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.24s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.26s
```

All four checks exit 0.

## Project Gates

**Gate 1 — Config Surface Sync:**
```
cargo test -p anvilml --features mock-hardware -- config_reference
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Gate 3 — Node Parity:** Not applicable — this task modifies an existing node's INPUT_SLOTS (adds `clip`), not a node type. The test file `worker/tests/test_parity.py` does not exist in this repository.

## Public API Delta

```
git diff HEAD -- worker/nodes/arch/diffusion/zit.py worker/nodes/sampler.py worker/tests/test_arch_zit.py worker/tests/test_nodes_sampler.py | grep "^+.*pub " | head -40
```
No new `pub` items introduced. This task modifies existing public interfaces (adds a parameter to `sample()`, adds a slot to `INPUT_SLOTS`) but does not introduce new `pub` items.

## Deviations from Plan

- **Python parameter default constraint:** The approved plan's signature `def sample(model, conditioning, clip, latent, steps, cfg, seed, device, cancel_flag, emit_progress, vae=None, *, pipeline_cache=None)` is invalid Python because `clip` has a default (`None`) but `latent`, `steps`, `cfg`, `seed`, `device`, `cancel_flag`, and `emit_progress` do not. In Python, a parameter with a default cannot precede one without a default. The implemented signature adds `= None` defaults to all parameters after `conditioning`: `clip: Any = None, latent: Any = None, steps: int = 4, cfg: float = 7.0, seed: int = 42, device: str = "cpu", cancel_flag: Any = None, emit_progress: Callable[[int, int], None] | None = None`. This is the minimal change to make the signature valid Python. All existing callers use keyword arguments, so this is backward-compatible.
- **Additional test fix:** `test_sampler_metadata_attributes` in `test_nodes_sampler.py` expected 6 INPUT_SLOTS; updated to expect 7 and added the `clip` slot spec check. This was not listed in the plan's Files Affected but was required to fix a test failure caused by the new `clip` slot.

## Blockers

None.
