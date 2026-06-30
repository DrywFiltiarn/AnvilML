# Implementation Report: P900-A10

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P900-A10                                          |
| Phase         | 900 — Spec-Drift & Logging Retrofit               |
| Description   | anvilml-core: fix ProvisioningState's variant names to match ANVILML_DESIGN.md |
| Implemented   | 2026-06-30T19:12:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Renamed two variants of `ProvisioningState` in `crates/anvilml-core/src/types/worker.rs` to match the design document specification: `InProgress` → `Provisioning`, `Complete` → `Ready`. Updated the corresponding test in `crates/anvilml-core/tests/worker_tests.rs` and the test catalogue in `docs/TESTS.md`. Bumped the crate patch version from 0.1.20 to 0.1.21. No new dependencies, no new `pub` items, no logging changes. All 159 workspace tests pass, all four platform cross-checks pass, all project gates pass.

## Resolved Dependencies

None. This task introduces no new dependencies and references no external crate types or APIs.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/types/worker.rs` | Rename `ProvisioningState::InProgress` → `Provisioning`, `Complete` → `Ready` |
| Modify | `crates/anvilml-core/tests/worker_tests.rs` | Update `test_provisioning_state_serde_snake_case` for renamed variants and new JSON strings |
| Modify | `crates/anvilml-core/Cargo.toml` | Bump patch version 0.1.20 → 0.1.21 |
| Modify | `docs/TESTS.md` | Update `test_provisioning_state_serde_snake_case` entry to reflect new variant names and JSON strings |

## Commit Log

```
 crates/anvilml-core/Cargo.toml              | 2 +-
 crates/anvilml-core/src/types/worker.rs     | 4 ++--
 crates/anvilml-core/tests/worker_tests.rs   | 4 ++--
 docs/TESTS.md                               | 4 ++--
 4 files changed, 7 insertions(+), 7 deletions(-)
```

## Test Results

```
     Running tests/worker_tests.rs (target/debug/deps/worker_tests-d5b63d2c5d75663f)

running 4 tests
test test_provisioning_state_serde_snake_case ... ok
test test_env_report_serde_roundtrip ... ok
test test_worker_info_construction_and_serde_roundtrip ... ok
test test_worker_status_serde_snake_case ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full workspace test suite: 159 tests passed, 0 failed across all crates.

## Format Gate

```
(cargo fmt --all -- --check returned exit 0, no output)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.05s

# 2. Mock-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 27.78s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 22.69s

# 4. Real-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 20.11s
```

All four platform cross-checks exited 0.

## Project Gates

```
# Gate 1 — Config Surface Sync
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Gate 1 passed. No config surface changes in this task.

## Public API Delta

```
(no output — no new pub items introduced)
```

No new `pub` items. The variant rename changes the enum's internal variants but does not add or remove any `pub` functions, structs, traits, or enums. The `#[serde(rename_all = "snake_case")]` attribute causes the JSON wire values to change automatically: `"in_progress"` → `"provisioning"`, `"complete"` → `"ready"`.

## Deviations from Plan

None. The implementation follows the approved plan exactly:
- `InProgress` → `Provisioning` (line 93 in worker.rs)
- `Complete` → `Ready` (line 95 in worker.rs)
- Test pairs updated in `test_provisioning_state_serde_snake_case`
- `test_env_report_serde_roundtrip` confirmed unchanged (uses `NotStarted`)
- Doc comments, derive attributes, and `#[serde(rename_all = "snake_case")]` left untouched
- Version bumped from 0.1.20 to 0.1.21
- TESTS.md entry updated to reflect new variant names

## Blockers

None.
