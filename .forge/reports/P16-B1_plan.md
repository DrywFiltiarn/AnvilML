# Plan Report: P16-B1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P16-B1                                      |
| Phase       | 16 — Live Events                              |
| Description | anvilml-server: AppState gains broadcaster field, wired from main.rs |
| Depends on  | P16-A4                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-09T14:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Connect the HTTP layer's WebSocket subscribers to the scheduler's event loop by giving `AppState` a `broadcaster: Arc<EventBroadcaster>` field, and wiring a single `EventBroadcaster` instance from `backend/src/main.rs` so both the scheduler's event loop (via `spawn_event_loop()`) and the HTTP layer's `AppState` share the identical broadcast channel. The acceptance criterion is that `cargo test -p anvilml-server --test state_tests` and `cargo build -p anvilml` both exit 0, with at least two new tests covering the broadcaster's construction and cloning semantics.

## Scope

### In Scope
- Add `broadcaster: Arc<EventBroadcaster>` field to `AppState` in `crates/anvilml-server/src/state.rs`.
- Update `crates/anvilml-server/src/state.rs` import to include `anvilml_ipc::EventBroadcaster`.
- Update `make_full_state()` helper in `crates/anvilml-server/tests/state_tests.rs` to construct and include an `EventBroadcaster` in the state.
- Add `test_app_state_broadcaster_constructs()` test in `state_tests.rs` — verifies the field is accessible and the Arc pointer is valid.
- Add `test_app_state_broadcaster_clone_shares()` test in `state_tests.rs` — verifies cloning AppState shares the same `Arc<EventBroadcaster>` allocation.
- Update `backend/src/main.rs` to construct one `EventBroadcaster::new()` before `AppState`, wrap it in `Arc`, and pass it into the `AppState` construction.
- Bump `anvilml-server` crate patch version from `0.1.11` → `0.1.12` in `crates/anvilml-server/Cargo.toml`.

### Out of Scope
- No `defers_to` entries exist for this task. The task context's "confirm/verify at ACT time" phrases are instructions to resolve-then-implement, not to defer.
- Remaining §13.2 fields (`hardware`, `env_report`) on `AppState` stay absent until a later task needs them.
- WebSocket handler (P16-C1, P16-C2) does not use the broadcaster yet — that wiring happens in later tasks.
- `EventBroadcaster` construction, API shape, and feature flags are not modified — only consumed.

## Existing Codebase Assessment

**What already exists:** `AppState` (Phase 15's P15-A1) currently holds seven `Arc`-wrapped fields: `config`, `node_registry`, `start_time`, `scheduler`, `workers`, `db`, and `artifact_store`. It is already `#[derive(Clone)]` and all fields follow the established `Arc<T>` pattern. The `EventBroadcaster` type exists in `anvilml-ipc` (Phase 7's P7-C1) as a thin wrapper around `tokio::sync::broadcast::Sender<WsEvent>` with `new()` and `publish()` methods. `spawn_event_loop()` (P16-A4 retrofit) already accepts `broadcaster: Arc<EventBroadcaster>` as its third parameter and calls `broadcaster.publish()` on every mapped event. The `WorkerPool` exposes `demux()` and `transport()` accessors. The `backend/src/main.rs` normal startup path constructs all subsystems, the scheduler, and `AppState`, but currently has no `EventBroadcaster` — the broadcaster is not yet wired into the startup sequence.

**Established patterns:** All `AppState` fields use `Arc<T>` for shared ownership. The `make_full_state()` test helper constructs a complete state with all fields. Tests verify `Arc` sharing via `std::ptr::eq(Arc::as_ptr(...), ...)`. Doc comments follow the `///` format with description paragraphs and `# Arguments`/`# Returns` sections. The `anvilml-server` crate's `Cargo.toml` already depends on `anvilml-ipc` (path dependency), so no new dependency declarations are needed.

**Gap between design doc and current source:** The design doc references `spawn_event_loop()` with a `transport: Arc<RouterTransport>` parameter, but the actual source code (after P16-A4) uses `demux: Arc<Demux>` — this is already reflected in the task context's NOTE. The plan correctly accounts for this by specifying `demux` as the parameter name.

## Resolved Dependencies

| Type   | Name              | Version verified | MCP source | Feature flags confirmed |
|--------|-------------------|-----------------|------------|------------------------|
| crate  | EventBroadcaster  | Already in workspace (anvilml-ipc 0.1.5) | Codebase inspection | n/a (internal crate, path dependency) |
| crate  | tokio::sync::broadcast | Already in workspace (tokio 1.52.3) | Codebase inspection (Cargo.toml) | n/a (used via anvilml-ipc, not directly) |

No new external dependencies are introduced. `EventBroadcaster` is already exported from `anvilml-ipc::ws::broadcaster` (via `pub use ws::broadcaster::EventBroadcaster` in `lib.rs`). The `anvilml-server` crate already declares `anvilml-ipc` as a path dependency.

## Approach

1. **Add `broadcaster` field to `AppState`** in `crates/anvilml-server/src/state.rs`:
   - Add `use anvilml_ipc::EventBroadcaster;` to the imports.
   - Add `pub broadcaster: Arc<EventBroadcaster>` as the eighth field of the `AppState` struct, after `artifact_store`.
   - Add a `///` doc comment: "Central event broadcaster for WebSocket subscribers. The same `Arc<EventBroadcaster>` instance is shared with the scheduler's event loop (spawn_event_loop), so HTTP-layer subscribers receive all events the scheduler publishes."

2. **Update `make_full_state()` test helper** in `crates/anvilml-server/tests/state_tests.rs`:
   - Add `use anvilml_ipc::EventBroadcaster;` to the imports.
   - Add `let broadcaster = Arc::new(EventBroadcaster::new());` before the `AppState` construction.
   - Add `broadcaster,` to the `AppState` struct literal.
   - This ensures all existing tests that call `make_full_state()` continue to compile with the new field.

3. **Add `test_app_state_broadcaster_constructs()` test** in `state_tests.rs`:
   - Construct `AppState` via `make_full_state()`.
   - Verify the `broadcaster` field is accessible and the `Arc` pointer is valid (non-null) via `Arc::as_ptr()`.
   - This parallels the existing `test_app_state_artifact_store_constructs()` pattern.

4. **Add `test_app_state_broadcaster_clone_shares()` test** in `state_tests.rs`:
   - Construct `AppState` via `make_full_state()`.
   - Clone it to `cloned`.
   - Assert `std::ptr::eq(Arc::as_ptr(&state.broadcaster), Arc::as_ptr(&cloned.broadcaster))` — verifying the clone shares the same `Arc<EventBroadcaster>` allocation.
   - This parallels the existing `test_app_state_artifact_store_clone_shares()` pattern.

5. **Wire `EventBroadcaster` from `main.rs`** in `backend/src/main.rs`:
   - After constructing `workers` (line 219) and before constructing the scheduler, add:
     ```rust
     // Construct the event broadcaster once and share it with both
     // the scheduler's event loop and AppState — two independently
     // constructed broadcasters would silently never see each other's
     // events.
     let broadcaster = Arc::new(anvilml_ipc::EventBroadcaster::new());
     ```
   - Add `broadcaster: Arc::clone(&broadcaster),` to the `AppState` struct literal (after `artifact_store`).
   - No change to the `spawn_event_loop()` call yet — that is handled by the ACT agent in a subsequent step where `spawn_event_loop()` is actually called from `main.rs` (it currently doesn't exist there; the call will be added by the ACT agent as part of the full wiring).

   Actually, re-reading `main.rs` more carefully: `spawn_event_loop()` is NOT currently called from `main.rs` — the function exists in `event_loop.rs` but the startup path does not invoke it. The task says "pass the same Arc instance into both JobScheduler's spawn_event_loop() call and AppState." Since `spawn_event_loop()` is not yet called from `main.rs`, this task's scope is to construct the `EventBroadcaster` and pass it into `AppState`. The scheduler wiring (passing it to `spawn_event_loop()`) will be completed when `spawn_event_loop()` is actually wired into `main.rs` by the ACT agent — the task context says "constructs one EventBroadcaster, shares the same Arc with both" which means we construct it here and make it available to both. The ACT agent will add the `spawn_event_loop()` call and pass the same Arc.

   Clarified approach: Construct the `EventBroadcaster` in `main.rs`, add it to `AppState`, and note that the scheduler wiring (passing it to `spawn_event_loop()`) is part of the same task's implementation — the ACT agent will add the `spawn_event_loop()` call and pass the same Arc.

   Revised step 5:
   - After constructing `workers` (line 219), construct `let broadcaster = Arc::new(anvilml_ipc::EventBroadcaster::new());`.
   - Add `broadcaster: Arc::clone(&broadcaster),` to the `AppState` struct literal.
   - Add a call to `spawn_event_loop()` after the dispatch loop handle is obtained, passing `Arc::clone(&scheduler)`, `workers.demux()`, `Arc::clone(&broadcaster)`, and `Arc::clone(&workers)` — matching the `spawn_event_loop()` signature from `event_loop.rs`.
   - Capture the returned `JoinHandle` as `event_loop_handle` (parallel to `dispatch_handle`).
   - Add `event_loop_handle.abort()` and `let _ = event_loop_handle.await;` in the graceful shutdown section (parallel to `dispatch_handle` handling).

6. **Bump `anvilml-server` crate version** in `crates/anvilml-server/Cargo.toml`:
   - Change `version = "0.1.11"` → `version = "0.1.12"`.

## Public API Surface

No new `pub` items are introduced. The only change is an additive field on an existing `pub struct`:

```rust
// In anvilml_server::AppState (crates/anvilml-server/src/state.rs):
pub struct AppState {
    // ... existing 7 fields ...
    /// Central event broadcaster for WebSocket subscribers.
    pub broadcaster: Arc<EventBroadcaster>,
}
```

The `EventBroadcaster` type is already `pub` in `anvilml_ipc`. The `spawn_event_loop()` function signature is unchanged:
```rust
pub fn spawn_event_loop(
    scheduler: Arc<JobScheduler>,
    demux: Arc<Demux>,
    broadcaster: Arc<EventBroadcaster>,
    workers: Arc<WorkerPool>,
) -> JoinHandle<()>
```

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | crates/anvilml-server/src/state.rs | Add `broadcaster: Arc<EventBroadcaster>` field to `AppState` struct and its import. |
| Modify | crates/anvilml-server/tests/state_tests.rs | Update `make_full_state()` helper; add 2 new tests for broadcaster construction and cloning. |
| Modify | backend/src/main.rs | Construct `EventBroadcaster`, wire into `AppState`, and call `spawn_event_loop()` with the same Arc. |
| Modify | crates/anvilml-server/Cargo.toml | Bump patch version 0.1.11 → 0.1.12. |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `crates/anvilml-server/tests/state_tests.rs` | `test_app_state_broadcaster_constructs` | The `broadcaster` field is accessible on `AppState` and the `Arc` pointer is valid (non-null). | `cargo test -p anvilml-server --test state_tests test_app_state_broadcaster_constructs` exits 0 |
| `crates/anvilml-server/tests/state_tests.rs` | `test_app_state_broadcaster_clone_shares` | Cloning `AppState` shares the same `Arc<EventBroadcaster>` allocation — verified via `std::ptr::eq(Arc::as_ptr(...))`. | `cargo test -p anvilml-server --test state_tests test_app_state_broadcaster_clone_shares` exits 0 |

## CI Impact

No CI changes required. The task modifies existing test files and source files within the existing crate structure. The existing CI jobs (`rust-linux`, `rust-windows`) already run `cargo test --workspace --features mock-hardware` which includes `anvilml-server` tests. The `config-drift` gate does not apply (no config fields changed). The `openapi-drift` gate does not apply (no handler signatures or `ToSchema` derives changed).

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. `EventBroadcaster` uses only `tokio::sync::broadcast::Sender`, which is cross-platform. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are required.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `spawn_event_loop()` is not yet called from `main.rs` — wiring it in requires understanding the correct call site (after workers are spawned, alongside dispatch loop). A wrong placement could cause the event loop to start before workers are ready, or miss the shutdown ordering. | Medium | High | Read `main.rs` carefully to find the correct insertion point: after `dispatch_handle` is created, before `build_router(app_state)`. Mirror the existing `dispatch_handle` pattern for the `event_loop_handle` in both startup and shutdown sections. |
| Adding the `broadcaster` field to `AppState` without updating `make_full_state()` would cause all existing tests to fail to compile (missing struct field). | High | Medium | Update `make_full_state()` in the same edit that adds the field to `AppState`, ensuring all test code compiles immediately. |
| The `EventBroadcaster::new()` constructor returns a non-`Debug` type (it derives `Debug` but wraps a `Sender<WsEvent>`). No issue expected — `EventBroadcaster` derives `Debug` in the source. | Low | Low | Confirmed via codebase inspection: `EventBroadcaster` derives `Debug` and has `pub fn new()`. No API mismatch. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server --test state_tests` exits 0
- [ ] `cargo build -p anvilml` exits 0
- [ ] `grep -c "pub broadcaster" crates/anvilml-server/src/state.rs` returns 1 (the field exists)
- [ ] `grep -c "test_app_state_broadcaster" crates/anvilml-server/tests/state_tests.rs` returns 2 (two new tests)
- [ ] `grep "EventBroadcaster" backend/src/main.rs` returns at least 1 match (broadcaster is constructed in main)
