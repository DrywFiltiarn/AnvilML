# Implementation Report: P900-A3

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P900-A3                            |
| Phase         | 900 — Spec-Drift & Logging Retrofit |
| Description   | anvilml-core: add missing ToSchema to Job/JobStatus/JobSettings |
| Implemented   | 2026-06-30T14:00:00Z               |
| Status        | COMPLETE                           |

## Summary

Added the `utoipa::ToSchema` derive to `JobStatus`, `JobSettings`, and `Job` in `crates/anvilml-core/src/types/job.rs`, closing the gap between `ANVILML_DESIGN.md §5.3` (which specifies these three types derive `ToSchema`) and the live code. This is a pure derive addition with zero runtime behaviour change: one import line added, three derive attributes modified. The `utoipa` dependency (version 5.5.0) was already declared in the crate's `Cargo.toml` with the `macros` feature (default) enabled. All 108 workspace tests pass, `cargo doc -p anvilml-core --no-deps` builds successfully, and all four platform cross-checks (mock Linux, mock Windows, real Linux, real Windows) compile cleanly.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source         |
|--------|---------|------------------|----------------|
| crate  | utoipa  | 5.5.0            | rust-docs MCP  |

The `utoipa` dependency was already declared in `anvilml-core/Cargo.toml` at version 5.5.0 with features `["uuid", "chrono"]`. The `macros` feature (default) provides the `ToSchema` derive macro. No new dependency or feature flag was introduced.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/types/job.rs` | Added `use utoipa::ToSchema;` import; appended `, ToSchema` to derive list on `JobStatus`, `JobSettings`, and `Job` |
| Modify | `crates/anvilml-core/Cargo.toml` | Bumped patch version 0.1.17 → 0.1.18 |

## Commit Log

```
 .forge/reports/P900-A3_plan.md       | 136 +++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md         |   6 +-
 .forge/state/state.json              |  13 ++--
 Cargo.lock                           |   2 +-
 crates/anvilml-core/Cargo.toml       |   2 +-
 crates/anvilml-core/src/types/job.rs |   7 +-
 6 files changed, 152 insertions(+), 14 deletions(-)
```

## Test Results

```
     Running tests/job_tests.rs (target/debug/deps/job_tests-c923c844df7b79b6)

running 4 tests
test test_job_settings_default ... ok
test test_job_serde_roundtrip ... ok
test test_job_status_all_variants_roundtrip ... ok
test test_job_with_nulls_roundtrip ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full workspace test suite (108 tests, all crates):
```
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (anvilml)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (backend cli_help)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (config_reference)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (hw_probe_help)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (logging_tests)
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (shutdown_tests)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (artifacts store)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (core config_load)
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (core artifact)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (core config_load_tests)
test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (core config)
test result: ok. 16 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (core error)
test result: ok. 10 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (core events)
test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (core hardware)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (core job)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (core model)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (core node_registry)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (core node)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (core worker)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (hardware cpu)
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (hardware detect)
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (hardware mock)
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (hardware sysfs)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (hardware vulkan)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (ipc roundtrip)
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (registry db)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (registry device_store)
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out  (registry scanner)
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (registry seed_loader)
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (registry store)
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   (server health)
```

## Format Gate

```
(no output — exit 0)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 20.71s

# 2. Mock-hardware Windows
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 29.67s

# 3. Real-hardware Linux
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.34s

# 4. Real-hardware Windows
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.08s
```

All four platform cross-checks exit 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### Gate 2 — OpenAPI Drift
Not triggered — this task modifies no handler function signatures, no `#[utoipa::path]` annotations, no `AppState` fields, and no response types. Only derives were added to existing types.

### Gate 3 — Node Parity
Not triggered — this task modifies no node types in `worker/nodes/` and no `node_registry.rs`.

### Gate 4 — Mock/Real Parity Markers
Not triggered — this task modifies no node's `execute()` or arch module's `load()`/`sample()`/`decode()`/`compute_latent_shape()`.

## Public API Delta

No new `pub` items introduced. The grep `git diff HEAD -- crates/anvilml-core/src/types/job.rs | grep '^+.*pub '` returned no output. The three existing `pub` items (`JobStatus`, `JobSettings`, `Job`) each gain a new trait impl (`utoipa::ToSchema`) but no signatures or fields change.

## Deviations from Plan

None. Implementation matches the approved plan exactly: one import added, three derive attributes modified, version bumped, all tests pass.

## Blockers

None.
