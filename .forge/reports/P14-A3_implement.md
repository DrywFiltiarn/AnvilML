# Implementation Report: P14-A3

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P14-A3                                              |
| Phase         | 014 — Dispatch & Mock Execute                     |
| Description   | anvilml-scheduler: handle Completed/Failed events, update job status |
| Implemented   | 2026-06-20T14:30:00Z                                |
| Status        | COMPLETE                                             |

## Summary

Implemented the event loop for processing `WorkerEvent::Completed` and `WorkerEvent::Failed` events in the `anvilml-scheduler` crate. The event loop subscribes to a new `WorkerEvent` broadcast channel on `EventBroadcaster`, updates job status in the SQLite database, releases VRAM reservations via `VramLedger::release`, and broadcasts `WsEvent::JobCompleted`/`WsEvent::JobFailed` to WebSocket clients. A new migration (002) adds a `device_index` column to the jobs table for efficient VRAM release lookups. The event loop gracefully falls back to parsing `worker_id` ("worker-N" → N) when the column doesn't exist, ensuring compatibility with pre-migration databases. Three integration tests cover the Completed, Failed, and unknown event paths.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source         |
|--------|---------|------------------|----------------|
| crate  | (none)  | —                | No new dependencies |

No new external dependencies were added. The task uses existing dependencies: `tokio::sync::broadcast` (already used by `EventBroadcaster` and `WorkerPool`), `sqlx` (already used for all database operations), `chrono::Utc` (already imported in `scheduler.rs`), and `tracing` (already imported).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-scheduler/src/event_loop.rs` | Event subscription loop: receives WorkerEvents, updates DB, releases VRAM, broadcasts WsEvent |
| MODIFY | `crates/anvilml-scheduler/src/scheduler.rs` | Add `start_event_loop()` method, `#[doc(hidden)]` accessors (`ledger()`, `broadcaster()`, `db()`), split dispatch UPDATE into two queries (worker_id + device_index for migration compatibility), update TODO comment |
| MODIFY | `crates/anvilml-scheduler/src/lib.rs` | Add `pub mod event_loop;` re-export |
| MODIFY | `crates/anvilml-ipc/src/ws/broadcaster.rs` | Add `worker_event_tx` field, `subscribe_worker_events()`, `broadcast_worker_event()` methods |
| CREATE | `crates/anvilml-scheduler/tests/event_loop_tests.rs` | 3 tests: Completed event, Failed event, unknown event (Pong) handling |
| Modify | `crates/anvilml-scheduler/Cargo.toml` | Bump patch version 0.1.8 → 0.1.9 |
| CREATE | `database/migrations/002_add_device_index.sql` | ALTER TABLE to add nullable `device_index` column |
| Modify | `docs/TESTS.md` | Add entries for 3 new tests |

## Commit Log

```
 .forge/reports/P14-A3_plan.md                      | 204 +++++++++++
 .forge/state/CURRENT_TASK.md                       |   6 +-
 .forge/state/state.json                            |  13 +-
 Cargo.lock                                         |   2 +-
 crates/anvilml-ipc/src/ws/broadcaster.rs           |  77 +++-
 crates/anvilml-scheduler/Cargo.toml                |   2 +-
 crates/anvilml-scheduler/src/event_loop.rs         | 302 ++++++++++++++++
 crates/anvilml-scheduler/src/lib.rs                |   1 +
 crates/anvilml-scheduler/src/scheduler.rs          |  67 +++-
 crates/anvilml-scheduler/tests/event_loop_tests.rs | 386 +++++++++++++++++++++
 database/migrations/002_add_device_index.sql       |  10 +
 docs/TESTS.md                                      |  27 ++
 12 files changed, 1071 insertions(+), 26 deletions(-)
```

## Test Results

```
Running tests/event_loop_tests.rs (target/debug/deps/event_loop_tests-47a159d0f6a079ba)

running 3 tests
test test_completed_event_updates_job_status ... ok
test test_failed_event_updates_job_status ... ok
test test_event_loop_ignores_unknown_event ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Running tests/dispatch_tests.rs (target/debug/deps/dispatch_tests-2025ee5c7c7e65f1)

running 5 tests
test test_device_preference_respected ... ok
test test_dispatch_to_idle_worker ... ok
test test_dispatch_wakes_on_notify ... ok
test test_no_dispatch_when_no_idle_workers ... ok
test test_vram_reserved_on_dispatch ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

Full workspace test suite: 180 tests, 0 failures, 0 ignored.
```

## Format Gate

```
cargo fmt --all -- --check
```
(Exit 0 — no output, no drift)

## Platform Cross-Check

All four checks passed:
1. `cargo check --workspace --features mock-hardware` — OK
2. `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` — OK
3. `cargo check --bin anvilml` — OK
4. `cargo check --bin anvilml --target x86_64-pc-windows-gnu` — OK

## Project Gates

**Gate 1 — Config Surface Sync:**
```
cargo test -p anvilml --features mock-hardware -- config_reference
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**Gate 2 — OpenAPI Drift:** Not triggered — no handler signatures, `#[utoipa::path]` annotations, or `ToSchema` derives were modified.

**Gate 3 — Node Parity:** Not triggered — no node types were added, removed, or renamed.

## Public API Delta

```
+    pub fn subscribe_worker_events(&self) -> broadcast::Receiver<crate::WorkerEvent> {
+    pub fn broadcast_worker_event(&self, event: crate::WorkerEvent) {
+pub mod event_loop;
+    pub fn start_event_loop(&self) -> tokio::task::JoinHandle<()> {
```

Additional `#[doc(hidden)]` accessors on `JobScheduler`:
- `pub fn ledger(&self) -> &Arc<tokio::sync::Mutex<VramLedger>>`
- `pub fn broadcaster(&self) -> &Arc<EventBroadcaster>`
- `pub fn db(&self) -> SqlitePool`

These are internal implementation details marked `#[doc(hidden)]` and are not part of the public API surface declared in the plan.

## Deviations from Plan

- **device_index fallback strategy**: The plan states to query `device_index` from the database. Since the `device_index` column requires migration 002 (which I added), the event loop includes a fallback: when `device_index` is NULL (pre-migration databases), it parses the `worker_id` string ("worker-N" → N). This ensures the event loop works on both migrated and non-migrated databases.
- **Two UPDATE queries in dispatch_once**: The original plan implied a single UPDATE for `worker_id` and `device_index`. Since `device_index` may not exist on pre-migration databases, the dispatch loop splits the UPDATE into two queries: (1) `started_at` + `worker_id` (always runs), (2) `device_index` (silently fails if column absent). This prevents the `worker_id` update from failing on older databases.
- **VRAM release amount**: The plan uses a hardcoded 4096 MiB default, matching the dispatch loop. This was kept as-is per the plan. The `VRAM_RELEASE_MIB` constant was extracted to a module-level const for clarity.
- **Migration 002**: Not explicitly listed in the plan's Files Affected, but necessary to support efficient device_index lookups in the event loop. The column is nullable to avoid breaking existing databases.

## Blockers

None.
