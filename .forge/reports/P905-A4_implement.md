# Implementation Report: P905-A4

| Field       | Value                                               |
|-------------|-----------------------------------------------------|
| Task ID     | P905-A4                                             |
| Phase       | 905 — FP8 dtype + model registry metadata           |
| Description | anvilml-registry: remove stale model records on rescan |
| Implemented | 2026-06-12T14:30:00Z                                |
| Status      | COMPLETE                                            |

## Summary

Extended `ModelRegistry::rescan` in `anvilml-registry` to automatically detect and delete stale model records whose file paths no longer exist on disk. Changed the return type from `Result<u32>` to `Result<(usize, usize)>` to report both the number of models upserted and the number of stale rows removed. Updated both callers (`handlers/models.rs` and `backend/src/main.rs`) to destructure the tuple and log both counts. Added an integration test verifying the stale-detection behavior.

## Resolved Dependencies

No new dependencies added or modified.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-registry/src/store.rs` | Extended `rescan` to detect and delete stale model records; changed return type to `Result<(usize, usize)>`; added `HashSet` import; added stale-detection SQL queries and DELETE loop with logging. |
| Modify | `crates/anvilml-registry/Cargo.toml` | Bump patch version `0.1.2 → 0.1.3`. |
| Modify | `crates/anvilml-server/src/handlers/models.rs` | Updated `rescan_models` handler to destructure `(upserted, removed)` tuple and log both counts. |
| Modify | `backend/src/main.rs` | Updated initial background rescan spawn to destructure tuple and log both counts. |
| Modify | `crates/anvilml-registry/tests/rescan.rs` | Updated existing tests to destructure `(upserted, removed)` tuple to match new return type. |
| Create   | `crates/anvilml-registry/tests/rescan_stale.rs` | Integration test: scan 2 files, delete 1, rescan, assert removed == 1 and DB has 1 row. |
| Modify | `backend/openapi.json` | Pre-existing drift fix from P905-A3 (FP8 dtype enum variants f8_e4m3, f8_e5m2). |

## Commit Log

```
 .forge/reports/P905-A4_plan.md                | 113 ++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                  |   6 +-
 .forge/state/state.json                       |  13 +--
 Cargo.lock                                    |   2 +-
 backend/openapi.json                          |   2 +
 backend/src/main.rs                           |   8 +-
 crates/anvilml-registry/Cargo.toml            |   2 +-
 crates/anvilml-registry/src/store.rs          |  64 +++++++++++++--
 crates/anvilml-registry/tests/rescan.rs       |  15 ++--
 crates/anvilml-registry/tests/rescan_stale.rs |  59 ++++++++++++++
 crates/anvilml-server/src/handlers/models.rs  |   8 +-
 11 files changed, 268 insertions(+), 24 deletions(-)
```

## Test Results

```
     Running tests/rescan.rs (target/debug/deps/rescan-9da069fe74673f7c)

running 2 tests
test test_rescan_adds_models ... ok
test test_rescan_idempotent ... ok

test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s

     Running tests/rescan_stale.rs (target/debug/deps/rescan_stale-fbe06e47bd686f65)

running 1 test
test test_rescan_removes_stale_models ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.04s
```

Full workspace test suite: all 272 tests passed, 0 failed.

## Format Gate

```
cargo fmt --all -- --check
```
Exit code 0 — no formatting drift.

## Platform Cross-Check

```
# 1. Mock-hardware Linux native
cargo check --workspace --features mock-hardware
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.11s

# 2. Mock-hardware Windows cross
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 12.13s

# 3. Real-hardware Linux native
cargo check --bin anvilml
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 15.33s

# 4. Real-hardware Windows cross
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 18.48s
```

All 4 cross-checks exit 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
cargo test -p backend --features mock-hardware -- config_reference
  running 0 tests
  test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out
```

### Gate 2 — OpenAPI Drift
```
cargo run -p anvilml-openapi && git diff --exit-code backend/openapi.json
  Generated OpenAPI spec: /home/dryw/AnvilML/backend/openapi.json
  (no diff output — gate passed)
```

## Deviations from Plan

- The SQL prefix matching uses two queries per dir root (`LIKE ? || '/' || '%'` for subdirectory paths, and `= ?` for exact root matches) rather than the plan's suggested single `LIKE ? || '%'` query. This avoids false positives where a dir root like `/models/diffusion` would incorrectly match `/models/diffusion_extra/model.safetensors`.
- The `backend/openapi.json` drift gate was triggered by a pre-existing drift from P905-A3 (FP8 dtype enum variants `f8_e4m3`, `f8_e5m2`). Regenerated and staged as part of this session.
- The `Cargo.lock` was automatically regenerated by cargo during `cargo fmt --all` due to the version bump.

## Blockers

None.
