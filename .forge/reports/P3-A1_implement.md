# Implementation Report: P3-A1

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P3-A1                                             |
| Phase         | 003 — Core Domain Types                           |
| Description   | anvilml-core: job types (Job, JobStatus, JobSettings, SubmitJobRequest/Response) |
| Implemented   | 2026-06-14T17:00:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Implemented five job domain types (`Job`, `JobStatus`, `JobSettings`, `SubmitJobRequest`, `SubmitJobResponse`) in `crates/anvilml-core/src/types/job.rs`, wired the new `types` module into `lib.rs`, added `chrono` and `utoipa` workspace dependencies, and wrote 5 integration tests. All 17 workspace tests pass, all 4 platform cross-checks pass, format and lint are clean.

## Resolved Dependencies

| Type   | Name     | Version resolved | Source         |
|--------|----------|------------------|----------------|
| crate  | chrono   | 0.4.45           | Plan (verified via crates.io API) |
| crate  | utoipa   | 5.5.0            | Plan (verified via crates.io API) |

Note: Added `uuid` feature to `utoipa` at implementation time because `Uuid` does not implement `ToSchema` without the `uuid` feature. This was discovered during `cargo check` — the plan did not include this feature.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/types/mod.rs` | Types module declaration and re-exports |
| CREATE | `crates/anvilml-core/src/types/job.rs` | Job, JobStatus, JobSettings, SubmitJobRequest, SubmitJobResponse (5 types, 98 lines) |
| MODIFY | `crates/anvilml-core/src/lib.rs` | Added `pub mod types;` and `pub use types::{...};` re-export |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Added `chrono` and `utoipa` workspace deps; bumped version 0.1.2 → 0.1.3 |
| MODIFY | `Cargo.toml` | Added `chrono` and `utoipa` to `[workspace.dependencies]` |
| CREATE | `crates/anvilml-core/tests/job_tests.rs` | 5 integration tests (roundtrip, defaults, status variants) |
| MODIFY | `docs/TESTS.md` | Added 5 entries for new job tests |
| MODIFY | `.forge/state/CURRENT_TASK.md` | Updated task state (The Forge state management) |
| MODIFY | `.forge/state/state.json` | Updated state (The Forge state management) |

## Commit Log

```
 .forge/state/CURRENT_TASK.md           |   6 +-
 .forge/state/state.json                |  13 ++--
 Cargo.lock                             | 134 ++++++++++++++++++++++++++++++++-
 Cargo.toml                             |   2 +
 crates/anvilml-core/Cargo.toml         |   4 +-
 crates/anvilml-core/src/lib.rs         |   2 +
 docs/TESTS.md                          |  40 ++++++++++++
 7 files changed, 190 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/job_tests.rs (target/debug/deps/job_tests-b171fad596e7394f)

running 5 tests
test test_job_settings_default ... ok
test test_job_json_roundtrip ... ok
test test_job_status_variants ... ok
test test_submit_job_request_default ... ok
test test_submit_job_response_default ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Full workspace: 17 tests passed, 0 failed, 0 ignored.
```

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
CHECK 1 — Mock-hardware Linux (cargo check --workspace --features mock-hardware):
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.26s

CHECK 2 — Mock-hardware Windows (cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu):
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.14s

CHECK 3 — Real-hardware Linux (cargo check --bin anvilml):
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.65s

CHECK 4 — Real-hardware Windows (cargo check --bin anvilml --target x86_64-pc-windows-gnu):
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.68s
```

All four checks pass.

## Project Gates

Gate 1 (Config Surface Sync) — not triggered. The `config_reference` test does not yet exist in the codebase. This task does not modify any `ServerConfig` fields, so the gate is not applicable.

## Public API Delta

```
+pub mod types;
+pub use types::{Job, JobSettings, JobStatus, SubmitJobRequest, SubmitJobResponse};
```

New public items introduced:

| Item | Type | Module Path |
|------|------|-------------|
| `Job` | struct | `anvilml_core::types::Job` |
| `JobStatus` | enum | `anvilml_core::types::JobStatus` |
| `JobSettings` | struct | `anvilml_core::types::JobSettings` |
| `SubmitJobRequest` | struct | `anvilml_core::types::SubmitJobRequest` |
| `SubmitJobResponse` | struct | `anvilml_core::types::SubmitJobResponse` |

All five match the plan's Public API Surface table exactly.

## Deviations from Plan

- **Added `PartialEq` and `Eq` derives to `JobSettings`**: The plan only listed `Debug, Clone, Serialize, Deserialize, ToSchema, Default`. The integration test `test_job_json_roundtrip` asserts `restored.settings == job.settings`, which requires `PartialEq`. The existing `config.rs` pattern also derives `PartialEq` on all config structs, so this follows the established convention.
- **Added `uuid` feature to `utoipa`**: The plan listed `features = ["macros", "chrono"]`. During `cargo check`, the `ToSchema` derive failed because `Uuid` does not implement `ToSchema` without the `uuid` feature. Added `uuid` to the feature list to resolve the compile error. This is a minimal fix — `utoipa` 5.5.0 provides a `uuid` feature that enables `Uuid` schema support.

## Blockers

None.
