# Implementation Report: P7-A2

| Field         | Value                           |
|---------------|---------------------------------|
| Task ID       | P7-A2                           |
| Phase         | 007 — IPC Foundations           |
| Description   | anvilml-ipc: WorkerMessage enum (Rust to Python) |
| Implemented   | 2026-06-30T20:15:00Z            |
| Status        | COMPLETE                        |

## Summary

Implemented the `WorkerMessage` enum in `crates/anvilml-ipc/src/messages.rs` with five variants (Ping, Shutdown, Execute, CancelJob, MemoryQuery) as specified in ANVILML_DESIGN.md §8.5. Added msgpack roundtrip tests for all five variants in `crates/anvilml-ipc/tests/roundtrip_tests.rs`. The enum uses `#[serde(tag = "_type")]` for discriminated msgpack encoding. All 175 workspace tests pass, all gates pass, and format/lint/cross-check are clean.

## Resolved Dependencies

| Type   | Name      | Version resolved | Source         |
|--------|-----------|------------------|----------------|
| crate  | rmp-serde | 1.3.1            | rust-docs MCP  |
| crate  | uuid      | 1.23.4           | rust-docs MCP  |
| crate  | serde     | 1.0 (latest)     | MCP fallback   |
| crate  | serde_json| 1.0 (latest)     | MCP fallback   |

Note: `serde` and `serde_json` were not initially direct dependencies of `anvilml-ipc` — the `messages.rs` module requires them as production dependencies (not just dev-dependencies) because the enum derives `Serialize`/`Deserialize` and uses `serde_json::Value` in the `Execute` variant. Added them to `[dependencies]`. The `uuid` dependency was also moved from `[dev-dependencies]` to `[dependencies]` with the `serde` feature, since it is used in the production code (`Execute::job_id`, `CancelJob::job_id`), not just tests.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-ipc/src/messages.rs` | `WorkerMessage` enum with 5 variants, doc comments, serde/msgpack derives, `PartialEq` |
| MODIFY | `crates/anvilml-ipc/src/lib.rs` | Added `pub mod messages;` declaration |
| MODIFY | `crates/anvilml-ipc/Cargo.toml` | Added `serde`, `serde_json`, `uuid` to `[dependencies]`; added `rmp-serde` to `[dev-dependencies]`; added `serde` feature to `uuid`; bumped version 0.1.2 → 0.1.3 |
| MODIFY | `crates/anvilml-ipc/tests/roundtrip_tests.rs` | Added 5 msgpack roundtrip tests for each `WorkerMessage` variant |
| MODIFY | `docs/TESTS.md` | Added 5 test catalogue entries for the new roundtrip tests |

## Commit Log

```
 .forge/reports/P7-A2_plan.md                | 147 ++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                |   6 +-
 .forge/state/state.json                     |  13 +--
 Cargo.lock                                  |  24 ++++-
 crates/anvilml-ipc/Cargo.toml               |   8 +-
 crates/anvilml-ipc/src/lib.rs               |   1 +
 crates/anvilml-ipc/src/messages.rs          |  59 +++++++++++
 crates/anvilml-ipc/tests/roundtrip_tests.rs |  82 ++++++++++++++++
 docs/TESTS.md                               |  60 ++++++++++++
 9 files changed, 388 insertions(+), 12 deletions(-)
```

## Test Results

```
running 9 tests
test test_cancel_job_roundtrip ... ok
test test_execute_roundtrip ... ok
test test_memory_query_roundtrip ... ok
test test_ping_roundtrip ... ok
test test_publish_multiple_subscribers_independent_copies ... ok
test test_publish_one_subscriber_delivers ... ok
test test_shutdown_roundtrip ... ok
test test_publish_zero_subscribers ... ok
test test_subscribe_returns_valid_receiver ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s
```

Full workspace test suite: 175 tests passed, 0 failed, 0 ignored.

## Format Gate

```
(no output — exit 0, no drift)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux:
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.73s

# 2. Mock-hardware Windows:
Finished `dev` profile [unoptimized + debuginfo] target(s) in 25.09s

# 3. Real-hardware Linux:
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.17s

# 4. Real-hardware Windows:
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.18s
```

All four cross-checks exit 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Gate 2 — OpenAPI Drift
Not triggered — this task does not modify handler function signatures, `#[utoipa::path]` annotations, or `AppState` fields used in response types.

## Public API Delta

```
+pub mod messages;
```

The only new `pub` item is `pub mod messages;` in `lib.rs`. The `WorkerMessage` enum itself is `pub` within the `messages` module but is NOT re-exported at the crate root — that is deferred to P7-D1 as specified in the plan.

## Deviations from Plan

1. **Added `serde`, `serde_json`, and `uuid` as production dependencies.** The plan assumed these came transitively through `anvilml-core`, but the `messages.rs` module uses `serde::{Serialize, Deserialize}`, `serde_json::Value`, and `uuid::Uuid` directly in production code — not just in tests. These crates are needed as `[dependencies]` (not `[dev-dependencies]`) for the enum to compile. The `uuid` dependency was also moved from `[dev-dependencies]` to `[dependencies]` with the `serde` feature.
2. **Added `PartialEq` derive to `WorkerMessage`.** The plan's Public API Surface table listed `Debug, Clone, Serialize, Deserialize` but omitted `PartialEq`. This derive is essential for the roundtrip tests to compare serialized/deserialized values via `assert_eq!`. It is a natural companion to the other derives for a data type.

## Blockers

None.
