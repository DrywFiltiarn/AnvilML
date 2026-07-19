# Implementation Report: P24-D2

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P24-D2                                            |
| Phase         | 024 — Generic Conditioning/Sampling/Decode Nodes, Real Mode |
| Description   | worker/nodes/image.py: SaveImage real branch encodes PNG, emits ImageReady |
| Implemented   | 2026-07-19T17:15:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Implemented the real branch of `SaveImage.execute()` in `worker/nodes/image.py` by replacing the `NotImplementedError` placeholder with real PNG encoding: takes the input `PIL.Image`, encodes it to PNG bytes via `BytesIO`, base64-encodes for the IPC payload, and emits an `ImageReady` event via `ctx.emit()` with all seven required fields (`job_id`, `image_b64`, `width`, `height`, `format`, `seed`, `steps`). Removed the `defers_to: P24-D2` comment markers. Added 6 new real-mode tests in `worker/tests/test_nodes_image.py` and updated `docs/TESTS.md` with entries for all new tests. All gates pass.

## Resolved Dependencies

None. This task uses only `PIL.Image` (Pillow, already a project dependency) and `base64` (Python standard library). No new external packages are introduced.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `worker/nodes/image.py` | Replace `NotImplementedError` with real PNG encoding + `ImageReady` emit; remove `defers_to: P24-D2` markers; update docstrings |
| MODIFY | `worker/tests/test_nodes_image.py` | Add 6 new real-mode tests for SaveImage real branch |
| MODIFY | `docs/TESTS.md` | Add 6 new test catalogue entries for real-mode SaveImage tests |

## Commit Log

```
 .forge/reports/P24-D2_plan.md    | 231 ++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md     |   6 +-
 .forge/state/state.json          |  13 ++-
 docs/TESTS.md                    |  60 ++++++++++
 worker/nodes/image.py            |  69 +++++++++---
 worker/tests/test_nodes_image.py | 236 +++++++++++++++++++++++++++++++++++++++
 6 files changed, 593 insertions(+), 22 deletions(-)
```

## Test Results

```
=== Rust Tests ===
cargo test --workspace --features mock-hardware
  All crates: 0 failed; all passed (300+ tests across all crates)

=== Python Mock-Mode Tests ===
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v -m "not real_mode"
  145 passed, 131 deselected

=== Python Real-Mode Tests ===
worker/.venv/bin/python -m pytest worker/tests/ -v -m real_mode
  131 passed, 145 deselected

=== New Real-Mode Tests (P24-D2) ===
worker/tests/test_nodes_image.py::test_save_image_real_emits_png PASSED
worker/tests/test_nodes_image.py::test_save_image_real_seed_pass_through PASSED
worker/tests/test_nodes_image.py::test_save_image_real_steps_pass_through PASSED
worker/tests/test_nodes_image.py::test_save_image_real_default_seed_steps PASSED
worker/tests/test_nodes_image.py::test_save_image_real_png_bytes_valid PASSED
worker/tests/test_nodes_image.py::test_save_image_real_returns_empty_dict PASSED
```

## Format Gate

```
cargo fmt --all -- --check
(no output — exit 0, no formatting drift)
```

## Platform Cross-Check

```
=== Check 1: Mock-hardware Linux ===
cargo check --workspace --features mock-hardware
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.32s

=== Check 2: Mock-hardware Windows ===
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 54.12s

=== Check 3: Real-hardware Linux ===
cargo check --bin anvilml
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.47s

=== Check 4: Real-hardware Windows ===
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.75s
```

## Project Gates

```
=== Gate 1: Config Surface Sync ===
cargo test -p anvilml --features mock-hardware -- config_reference
  test tests::config_reference_matches_defaults ... ok
  test result: ok. 1 passed; 0 failed

=== Gate 4: Mock/Real Parity Markers ===
grep -rn "REAL_PATH_VERIFIED:\|MOCK_PATH_VERIFIED:" worker/nodes/ | ...
  test_save_image_real_emits_png — collected
  test_save_image_mock_emits_image_ready — collected
grep -L "REAL_PATH_VERIFIED:" worker/nodes/**/*.py | grep -v __init__ | grep -v base.py
  (empty — no files missing REAL_PATH_VERIFIED marker)
grep -L "MOCK_PATH_VERIFIED:" worker/nodes/**/*.py | grep -v __init__ | grep -v base.py
  (empty — no files missing MOCK_PATH_VERIFIED marker)
```

## Public API Delta

```
git diff HEAD -- worker/nodes/image.py worker/tests/test_nodes_image.py | grep '^+.*pub ' | head -40
(no output — no new pub items introduced)
```

No new public items introduced. The existing `SaveImage.execute()` method signature is unchanged.

## Deviations from Plan

None. Implementation matches the approved plan exactly:
- Real branch replaces `NotImplementedError` with real PNG encoding + base64 + `ImageReady` emit.
- `defers_to: P24-D2` markers removed from both comment blocks.
- 6 new real-mode tests written (5 planned + 1 additional `test_save_image_real_returns_empty_dict` for completeness).
- Both `REAL_PATH_VERIFIED` and `MOCK_PATH_VERIFIED` markers confirm passing tests.
- `docs/TESTS.md` updated with entries for all 6 new tests.

## Blockers

None.
