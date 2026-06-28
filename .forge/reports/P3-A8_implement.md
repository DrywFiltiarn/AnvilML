# Implementation Report: P3-A8

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P3-A8                                       |
| Phase         | 003 — Core Domain Types: Data Model         |
| Description   | anvilml-core: WsEvent job-lifecycle variants |
| Implemented   | 2026-06-28T20:15:00Z                        |
| Status        | COMPLETE                                    |

## Summary

Created the `WsEvent` enum in `crates/anvilml-core/src/types/events.rs` with seven job-lifecycle variants (JobQueued, JobStarted, JobProgress, JobImageReady, JobCompleted, JobFailed, JobCancelled) per ANVILML_DESIGN.md §5.8. Wired the module into the crate's public API via `types/mod.rs`. Created integration tests in `crates/anvilml-core/tests/events_tests.rs` — one per variant — asserting serde tag roundtrip. All 7 tests pass. The enum derives `Debug`, `Clone`, `PartialEq`, `Serialize`, `Deserialize`, and `ToSchema`, with `#[serde(tag = "type", rename_all = "snake_case")]`.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source         |
|--------|---------|------------------|----------------|
| crate  | utoipa  | 5.5.0            | rust-docs MCP  |

No new dependencies added. The `ToSchema` derive macro is provided by the existing `utoipa` dependency (declared in `Cargo.toml` with `uuid` and `chrono` features).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/types/events.rs` | WsEvent enum with seven job-lifecycle variants, doc comments, `#[serde(tag = "type", rename_all = "snake_case")]` |
| MODIFY | `crates/anvilml-core/src/types/mod.rs` | Added `pub mod events;` and `pub use events::*;` |
| CREATE | `crates/anvilml-core/tests/events_tests.rs` | 7 integration tests, one per WsEvent variant, serde roundtrip assertions |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Bumped version 0.1.12 → 0.1.13 (then 0.1.13 → 0.1.14 after adding `PartialEq` derive) |
| MODIFY | `docs/TESTS.md` | Added 7 test catalogue entries for the new events tests |

## Commit Log

```
 .forge/reports/P3-A8_plan.md              | 136 +++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md              |   6 +-
 .forge/state/state.json                   |  13 ++-
 Cargo.lock                                |   2 +-
 crates/anvilml-core/Cargo.toml            |   2 +-
 crates/anvilml-core/src/types/events.rs   | 110 +++++++++++++++++++
 crates/anvilml-core/src/types/mod.rs      |   2 +
 crates/anvilml-core/tests/events_tests.rs | 176 ++++++++++++++++++++++++++++++
 docs/TESTS.md                             |  84 ++++++++++++++
 9 files changed, 520 insertions(+), 11 deletions(-)
```

## Test Results

```
running 7 tests
test test_ws_event_job_cancelled_serde_roundtrip ... ok
test test_ws_event_job_completed_serde_roundtrip ... ok
test test_ws_event_job_failed_serde_roundtrip ... ok
test test_ws_event_job_image_ready_serde_roundtrip ... ok
test test_ws_event_job_progress_serde_roundtrip ... ok
test test_ws_event_job_queued_serde_roundtrip ... ok
test test_ws_event_job_started_serde_roundtrip ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Full workspace test suite: 106 passed, 0 failed across all crates.

## Format Gate

```
(no output — all files already formatted)
```

## Platform Cross-Check

```
CHECK 1: OK — cargo check --workspace --features mock-hardware (Linux native)
CHECK 2: OK — cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
CHECK 3: OK — cargo check --bin anvilml (real-hardware Linux)
CHECK 4: OK — cargo check --bin anvilml --target x86_64-pc-windows-gnu (real-hardware Windows)
```

All four platform cross-check commands exit 0.

## Project Gates

None applicable — task does not touch config fields, handler signatures, or node types.

## Public API Delta

```
+pub mod events;
+pub use events::*;
```

New public items (from the new file `events.rs`):
- `pub enum WsEvent` — `anvilml_core::types::WsEvent`
  - Variant `JobQueued { job_id: Uuid, queue_position: u32 }`
  - Variant `JobStarted { job_id: Uuid, worker_id: String }`
  - Variant `JobProgress { job_id: Uuid, step: u32, total_steps: u32, preview_b64: Option<String> }`
  - Variant `JobImageReady { job_id: Uuid, artifact_hash: String, width: u32, height: u32, seed: i64, steps: u32 }`
  - Variant `JobCompleted { job_id: Uuid, elapsed_ms: u64 }`
  - Variant `JobFailed { job_id: Uuid, error: String }`
  - Variant `JobCancelled { job_id: Uuid }`

## Deviations from Plan

- Added `PartialEq` to the `#[derive(...)]` list on `WsEvent`. The approved plan listed `#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]` but the test file uses `assert_eq!` to compare roundtripped values against originals, which requires `PartialEq`. This matches the pattern used by all other type modules in the crate (e.g. `Job`, `ArtifactMeta`, `GpuDevice` all derive `PartialEq`).
- Version bumped to 0.1.14 (not 0.1.13) because the `PartialEq` addition is a source change to `events.rs` that required a second bump.

## Blockers

None.
