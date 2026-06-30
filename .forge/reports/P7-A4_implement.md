# Implementation Report: P7-A4

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P7-A4                           |
| Phase         | 007 — IPC Foundations           |
| Description   | anvilml-ipc: WorkerEvent job-lifecycle variants |
| Implemented   | 2026-06-30T21:25:00Z            |
| Status        | COMPLETE                        |

## Summary

Extended the `WorkerEvent` enum in `crates/anvilml-ipc/src/messages.rs` with five job-lifecycle variants (`Progress`, `ImageReady`, `Completed`, `Failed`, `Cancelled`) matching the exact field names, types, and order from `ANVILML_DESIGN.md §8.6`. Updated the module-level doc comment to remove the P7-A4 deferred note and include "job-lifecycle events" in the event categories list. Added `pub use messages::{WorkerEvent, WorkerMessage};` to `lib.rs`. Added five msgpack roundtrip tests in `roundtrip_tests.rs`. Bumped crate version from 0.1.4 to 0.1.5. All 18 roundtrip tests pass; full workspace test suite exits 0.

## Resolved Dependencies

No new dependencies introduced. Existing dependencies (`uuid` v1.23.4 with `serde`+`v4` features, `rmp-serde` v1.3.1) already cover all type and serialization requirements.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | crates/anvilml-ipc/src/messages.rs | Added 5 WorkerEvent job-lifecycle variants with doc comments; updated module-level doc comment |
| Modify | crates/anvilml-ipc/src/lib.rs | Added `pub use messages::{WorkerEvent, WorkerMessage};` |
| Modify | crates/anvilml-ipc/tests/roundtrip_tests.rs | Added 5 msgpack roundtrip tests (Progress, ImageReady, Completed, Failed, Cancelled) |
| Modify | crates/anvilml-ipc/Cargo.toml | Bumped patch version 0.1.4 → 0.1.5 |
| Modify | docs/TESTS.md | Added 5 test catalogue entries for new roundtrip tests |

## Commit Log

```
 .forge/reports/P7-A4_plan.md                | 174 ++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                |   6 +-
 .forge/state/state.json                     |  13 ++-
 Cargo.lock                                  |   2 +-
 crates/anvilml-ipc/Cargo.toml               |   2 +-
 crates/anvilml-ipc/src/lib.rs               |   1 +
 crates/anvilml-ipc/src/messages.rs          |  80 ++++++++++++-
 crates/anvilml-ipc/tests/roundtrip_tests.rs | 102 ++++++++++++++++
 docs/TESTS.md                               |  60 ++++++++++
 9 files changed, 427 insertions(+), 13 deletions(-)
```

## Test Results

```
running 18 tests
test test_cancel_job_roundtrip ... ok
test test_cancelled_roundtrip ... ok
test test_completed_roundtrip ... ok
test test_dying_roundtrip ... ok
test test_execute_roundtrip ... ok
test test_failed_roundtrip ... ok
test test_image_ready_roundtrip ... ok
test test_memory_query_roundtrip ... ok
test test_memory_report_roundtrip ... ok
test test_ping_roundtrip ... ok
test test_pong_roundtrip ... ok
test test_progress_roundtrip ... ok
test test_ready_roundtrip ... ok
test test_shutdown_roundtrip ... ok
test test_publish_one_subscriber_delivers ... ok
test test_publish_zero_subscribers ... ok
test test_publish_multiple_subscribers_independent_copies ... ok
test test_subscribe_returns_valid_receiver ... ok

test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.11s
```

Full workspace test suite: all crates compiled and all tests passed (0 failures across all crates).

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, no drift)
```

## Platform Cross-Check

```
Check 1 (mock-hardware Linux):      Finished `dev` profile — ok
Check 2 (mock-hardware Windows):    Finished `dev` profile — ok
Check 3 (real-hardware Linux):      Finished `dev` profile — ok
Check 4 (real-hardware Windows):    Finished `dev` profile — ok
```

All four platform cross-checks passed.

## Project Gates

Not triggered — this task does not modify `ServerConfig` fields (Gate 1), handler function signatures or `ToSchema` derives (Gate 2), node types (Gate 3), or node/arch-module `execute()`/`load()`/`sample()`/`decode()`/`compute_latent_shape()` functions (Gate 4).

## Public API Delta

```
+pub use messages::{WorkerEvent, WorkerMessage};
```

The only new `pub` item is the re-export in `lib.rs`. The five new enum variants (`Progress`, `ImageReady`, `Completed`, `Failed`, `Cancelled`) are variants of the existing `pub enum WorkerEvent` — they extend the public API surface as intended but are not themselves `pub` items.

## Deviations from Plan

None. Implementation followed the approved plan exactly:
- All five variants use the exact field names, types, and order from `ANVILML_DESIGN.md §8.6`.
- Each variant has a `///` doc comment matching the style of existing variants.
- Module-level doc comment updated to remove the deferred note and include "job-lifecycle events".
- Re-export added after `pub mod messages;` (which already existed).
- Five roundtrip tests follow the exact pattern of existing tests.
- Version bumped from 0.1.4 to 0.1.5.

## Blockers

None.
