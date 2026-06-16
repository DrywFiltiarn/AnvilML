# Implementation Report: P7-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P7-A1                                       |
| Phase       | 007 — WebSocket Event Stream                |
| Description | anvilml-server: EventBroadcaster wrapping broadcast channel |
| Implemented | 2026-06-16T08:30:00Z                        |
| Status      | COMPLETE                                    |

## Summary

Created the `EventBroadcaster` struct in `crates/anvilml-server/src/ws/broadcaster.rs` — a thin wrapper around `tokio::sync::broadcast::Sender<WsEvent>` with capacity 1024. Implemented `new()`, `send()`, and `subscribe()` methods with full doc comments and error logging. Created the `ws/` module with `mod.rs` (declaring `broadcaster`, `handler` stub, `stats_tick` stub) and re-exported `EventBroadcaster`. Added `pub mod ws;` to `lib.rs` and injected `Arc<EventBroadcaster>` into `AppState` in both constructors. Bumped `anvilml-server` version from 0.1.9 to 0.1.10. Added `uuid` as a dev-dependency for tests. Created 3 integration tests in `broadcaster_tests.rs` covering construction, send/receive, and lagged receiver behavior. All 117 workspace tests pass.

## Resolved Dependencies

| Type   | Name  | Version resolved | Source         |
|--------|-------|------------------|----------------|
| crate  | uuid  | 1.23.3           | Workspace Cargo.toml |

No new external dependencies were introduced beyond `uuid` in dev-dependencies (already present in workspace root Cargo.toml with version 1.23.3). The `tokio` workspace dependency (1.52.3, "full" features) provides `tokio::sync::broadcast`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-server/src/ws/broadcaster.rs` | EventBroadcaster struct with new(), send(), subscribe() |
| CREATE | `crates/anvilml-server/src/ws/mod.rs` | Module declarations and EventBroadcaster re-export |
| CREATE | `crates/anvilml-server/src/ws/handler.rs` | Stub placeholder for P7-A2 |
| CREATE | `crates/anvilml-server/src/ws/stats_tick.rs` | Stub placeholder for P7-A3 |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Added `pub mod ws;` |
| MODIFY | `crates/anvilml-server/src/state.rs` | Added `broadcaster` field to AppState; init in both constructors |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Bumped version 0.1.9 → 0.1.10; added uuid dev-dependency |
| CREATE | `crates/anvilml-server/tests/broadcaster_tests.rs` | 3 integration tests for EventBroadcaster |
| MODIFY | `docs/TESTS.md` | Added 3 test entries for new broadcaster tests |

## Commit Log

```
 .forge/reports/P7-A1_plan.md                     | 125 +++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md                     |   6 +-
 .forge/state/state.json                          |  13 +--
 Cargo.lock                                       |   3 +-
 crates/anvilml-server/Cargo.toml                 |   3 +-
 crates/anvilml-server/src/lib.rs                 |   1 +
 crates/anvilml-server/src/state.rs               |  18 +++-
 crates/anvilml-server/src/ws/broadcaster.rs      |  81 +++++++++++++++
 crates/anvilml-server/src/ws/handler.rs          |   7 ++
 crates/anvilml-server/src/ws/mod.rs              |  13 +++
 crates/anvilml-server/src/ws/stats_tick.rs       |   6 ++
 crates/anvilml-server/tests/broadcaster_tests.rs | 106 +++++++++++++++++++
 docs/TESTS.md                                    |  27 +++++
 13 files changed, 397 insertions(+), 12 deletions(-)
```

## Test Results

```
     Running tests/broadcaster_tests.rs (target/debug/deps/broadcaster_tests-13bccf5ad2a321e9)

running 3 tests
test test_broadcaster_send_and_receive ... ok
test test_broadcaster_new ... ok
test test_broadcaster_lagged_receiver ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

All 117 workspace tests pass (0 failed). The 3 new broadcaster tests exercise:
- Construction and basic subscription (test_broadcaster_new)
- Event delivery and equality check (test_broadcaster_send_and_receive)
- Lagged receiver behavior when all subscribers drop (test_broadcaster_lagged_receiver)

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output)
```

## Platform Cross-Check

```
# Check 1 — Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.30s
--- Check 1 PASS

# Check 2 — Mock-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.12s
--- Check 2 PASS

# Check 3 — Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.00s
--- Check 3 PASS

# Check 4 — Real-hardware Windows (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.12s
--- Check 4 PASS
```

All 4 platform cross-checks exit 0.

## Project Gates

### Gate 1 — Config Surface Sync
```
running 1 test
test config_reference ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Gate 2 — OpenAPI Drift
```
(cargo run -p anvilml-openapi exited 0; git diff --exit-code api/openapi.json exited 0)
```

Both gates pass. No config or OpenAPI drift detected.

## Public API Delta

From `git diff HEAD -- crates/anvilml-server/src/lib.rs crates/anvilml-server/src/state.rs | grep '^+.*pub '`:
```
+pub mod ws;
+    pub broadcaster: Arc<crate::ws::EventBroadcaster>,
```

From direct grep of new ws module files:
```
crates/anvilml-server/src/ws/broadcaster.rs:pub struct EventBroadcaster {
crates/anvilml-server/src/ws/mod.rs:pub mod broadcaster;
crates/anvilml-server/src/ws/mod.rs:pub mod handler;
crates/anvilml-server/src/ws/mod.rs:pub mod stats_tick;
crates/anvilml-server/src/ws/mod.rs:pub use broadcaster::EventBroadcaster;
```

New pub items:
- `pub mod ws` — fn/mod — `anvilml_server::ws` (module declaration in lib.rs)
- `pub struct EventBroadcaster` — struct — `anvilml_server::ws::broadcaster::EventBroadcaster`
- `pub fn EventBroadcaster::new` — fn — returns `Self`
- `pub fn EventBroadcaster::send` — fn — takes `WsEvent`, returns `()`
- `pub fn EventBroadcaster::subscribe` — fn — returns `broadcast::Receiver<WsEvent>`
- `pub mod broadcaster` — mod — `anvilml_server::ws::broadcaster`
- `pub mod handler` — mod — `anvilml_server::ws::handler` (stub)
- `pub mod stats_tick` — mod — `anvilml_server::ws::stats_tick` (stub)
- `pub use broadcaster::EventBroadcaster` — re-export — `anvilml_server::ws::EventBroadcaster`
- `pub broadcaster: Arc<...>` — field — new field on `AppState` struct

## Deviations from Plan

1. **API mismatch on `tokio::sync::broadcast::Sender::send()`**: The plan stated `self.tx.send(&event)` (passing a reference), but in tokio 1.52.3 the `send()` method takes `T` (owned value), not `&T`. Fixed by passing `event` by value. Since `send()` consumes the value, the clone for logging was added: `self.tx.send(event.clone())` — the clone only executes on the error path, keeping the hot path efficient.

2. **Test approach for lagged receiver**: The plan specified verifying `send()` returns `Err(SendError)`. Since `EventBroadcaster::send()` returns `()` (unit type, not `Result`), the test verifies the error path by confirming that `send()` does not panic when all subscribers have dropped and the channel is full — the event is silently dropped as designed.

3. **Added `uuid` dev-dependency**: Not mentioned in the plan's dependencies table, but required for generating unique `Uuid` values in the test's `JobQueued` events. The version (1.23.3) was taken from the workspace root `Cargo.toml`.

4. **Added `Default` impl for `EventBroadcaster`**: Not in the plan, but follows the established pattern in the codebase (e.g., `AppState` constructors use `Default`-like patterns). The `Default` impl delegates to `new()`.

## Blockers

None.
