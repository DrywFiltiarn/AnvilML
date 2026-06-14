# Implementation Report: P3-A5

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P3-A5                              |
| Phase         | 003 — Core Domain Types            |
| Description   | anvilml-core: WsEvent enum with all variant subtypes |
| Implemented   | 2026-06-14T22:15:00Z               |
| Status        | COMPLETE                           |

## Summary

Created `crates/anvilml-core/src/types/events.rs` containing the `WsEvent` tagged enum with all ten sub-event variants per `ANVILML_DESIGN.md §5.8`. Updated `types/mod.rs` to declare and re-export the events module, and `lib.rs` to add `WsEvent` to the top-level re-exports. Created four integration tests in `crates/anvilml-core/tests/events_tests.rs` verifying JSON roundtrips and the `"type"` discriminator. Bumped `anvilml-core` patch version from `0.1.7` to `0.1.8`. Also added `PartialEq`/`Eq` derives to `WorkerInfo` (required by `WsEvent`'s `SystemStats` variant containing `Vec<WorkerInfo>`) and to `WsEvent` itself (required by the all-variants roundtrip test).

## Resolved Dependencies

None. All dependencies (`serde`, `serde_json`, `utoipa`, `uuid`) are already declared in `crates/anvilml-core/Cargo.toml` as workspace dependencies. No new external crates were added.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/types/events.rs` | WsEvent enum with all 10 sub-event variants, doc comments, derives |
| CREATE | `crates/anvilml-core/tests/events_tests.rs` | 4 integration tests for WsEvent JSON roundtrips |
| MODIFY | `crates/anvilml-core/src/types/mod.rs` | Added `pub mod events;` and `pub use events::WsEvent;` |
| MODIFY | `crates/anvilml-core/src/lib.rs` | Added `WsEvent` to top-level `pub use types::` re-exports |
| MODIFY | `crates/anvilml-core/src/types/worker.rs` | Added `PartialEq, Eq` to `WorkerInfo` derive (required for `WsEvent::PartialEq`) |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Bumped patch version 0.1.7 → 0.1.8 |
| MODIFY | `docs/TESTS.md` | Added 4 entries for new events tests |

## Commit Log

```
 .forge/reports/P3-A5_plan.md              | 137 +++++++++++++++++
 .forge/state/CURRENT_TASK.md              |   6 +-
 .forge/state/state.json                   |  13 +-
 Cargo.lock                                |   2 +-
 crates/anvilml-core/Cargo.toml            |   2 +-
 crates/anvilml-core/src/lib.rs            |   2 +-
 crates/anvilml-core/src/types/events.rs   | 162 ++++++++++++++++++++
 crates/anvilml-core/src/types/mod.rs      |   2 +
 crates/anvilml-core/src/types/worker.rs   |   2 +-
 crates/anvilml-core/tests/events_tests.rs | 236 ++++++++++++++++++++++++++++++
 docs/TESTS.md                             |  32 ++++
 11 files changed, 583 insertions(+), 13 deletions(-)
```

## Test Results

```
     Running tests/events_tests.rs (target/debug/deps/events_tests-1af5b9f6badc4aeb)

running 4 tests
test test_ws_event_roundtrip_job_image_ready ... ok
test test_ws_event_all_variants_roundtrip ... ok
test test_ws_event_tag_field_present ... ok
test test_ws_event_system_stats_roundtrip ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace: 47 tests passed, 0 failed across all crates.

## Format Gate

```
cargo fmt --all -- --check
```
(No output — all files already formatted.)

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.41s

# 2. Mock-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.76s

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.65s

# 4. Real-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.68s
```

All four cross-checks exited 0.

## Project Gates

- Gate 1 (Config Surface Sync): Not triggered — task does not modify `ServerConfig` or nested config structs.
- Gate 2 (OpenAPI Drift): Not triggered — task does not modify handler signatures, `#[utoipa::path]` annotations, or `AppState` fields.
- Gate 3 (Node Parity): Not triggered — task does not modify node types or node registry.

## Public API Delta

```
+pub enum WsEvent {
+pub mod events;
+pub use events::WsEvent;
```

New public items:
- `pub mod events` — module path: `anvilml_core::types::events`
- `pub use events::WsEvent` — re-exported at crate root: `anvilml_core::WsEvent`
- `pub enum WsEvent` — with 10 public struct variants: `JobQueued`, `JobStarted`, `JobProgress`, `JobImageReady`, `JobCompleted`, `JobFailed`, `JobCancelled`, `WorkerStatusChanged`, `SystemStats`, `ProvisioningProgress`

## Deviations from Plan

- Added `PartialEq, Eq` derives to `WorkerInfo` in `worker.rs` (not mentioned in plan). Required because `WsEvent` derives `PartialEq` (needed by the all-variants roundtrip test) and `SystemStats` contains `Vec<WorkerInfo>` — `Vec<T>` requires `T: PartialEq`.
- Added `PartialEq` (but not `Eq`) to `WsEvent` derive (plan said `Debug, Clone, Serialize, Deserialize, ToSchema` only). `Eq` was excluded because `SystemStats` contains `cpu_pct: f32`, and `f32` does not implement `Eq` (NaN != NaN).
- Removed the `chrono` import from the plan's import list — confirmed no `DateTime<Utc>` fields exist in any WsEvent variant, consistent with the plan's note that `chrono` is not needed.

## Blockers

None.
