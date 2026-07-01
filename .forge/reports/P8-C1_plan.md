# Plan Report: P8-C1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P8-C1                                             |
| Phase       | 008 — IPC Stress Gate & Worker Pool               |
| Description | anvilml-worker: demux.rs register/deregister pair (mandatory) |
| Depends on  | P8-A1                                             |
| Project     | anvilml                                           |
| Planned at  | 2026-07-01T09:20:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create `crates/anvilml-worker/src/demux.rs` implementing the `Demux` struct — a thread-safe event routing table that maps `worker_id` to `mpsc::Sender<WorkerEvent>`. The `Demux` must expose a `register()` / `deregister()` pair (closing the v3 regression where only `register()` existed, causing routing entries to leak on every crash+respawn) and a `route()` method that dispatches events to the correct worker's sender or returns `AnvilError::WorkerNotFound`. Additionally, declare `mod demux; pub use demux::Demux;` in `lib.rs`. The task is accepted when `cargo test -p anvilml-worker --test demux_tests` exits 0 with ≥5 tests including the mandatory deregistration test.

## Scope

### In Scope
- **`crates/anvilml-worker/src/demux.rs`**: Create the `Demux` struct with:
  - `register(&self, worker_id: String, tx: mpsc::Sender<WorkerEvent>)` — inserts a new routing entry; returns `()` (idempotent: overwrites existing entry if present).
  - `deregister(&self, worker_id: &str) -> bool` — removes a routing entry; returns `true` if an entry existed and was removed, `false` otherwise (safe to call on absent entries).
  - `route(&self, worker_id: &str, event: WorkerEvent) -> Result<(), AnvilError>` — looks up the worker_id; if present, clones the sender and calls `send()` on it; if absent, returns `Err(AnvilError::WorkerNotFound(worker_id))`.
  - A private `inner: Mutex<HashMap<String, mpsc::Sender<WorkerEvent>>>` field.
- **`crates/anvilml-worker/src/lib.rs`**: Add `mod demux;` and `pub use demux::Demux;`.
- **`crates/anvilml-worker/Cargo.toml`**: Add `sync` feature to the existing `tokio` dependency to enable `tokio::sync::mpsc`.
- **`crates/anvilml-worker/tests/demux_tests.rs`**: Create integration test file with ≥5 tests.

### Out of Scope
None. `defers_to (from JSON): []` — this task has no deferrals. The register/deregister pair, the route method, and all tests are implemented in full within this task. No stubs, no "confirm at ACT time" deferred work.

## Existing Codebase Assessment

The `anvilml-worker` crate currently has 12 lines in `lib.rs` declaring `mod env`, `mod spawn`, and `mod job_object` (Windows-only). Two test files exist: `env_tests.rs` (7 tests, full coverage of `WorkerEnv::build()`) and `spawn_tests.rs` (7 tests, covering `build_command()` and Windows `JobObjectGuard`). The crate's `Cargo.toml` declares `tokio` with only the `process` feature — the `sync` feature needed for `mpsc` is absent.

The established test style in this crate is integration tests in `tests/` (not inline `#[cfg(test)]` blocks), using the crate's public API, with doc comments on each test explaining the invariant being verified. The `env_tests.rs` pattern of constructing a value and asserting on its fields is the model to follow.

The `WorkerEvent` enum (9 variants) lives in `anvilml-ipc::messages` and derives `Clone` + `Serialize` + `Deserialize`. The `AnvilError::WorkerNotFound(String)` variant already exists in `anvilml-core::error` and maps to HTTP 404. No gap exists between the design doc and current source for this task's types — all types are already defined and accessible.

The design doc (§9.4) explicitly states the v3 regression: `register()` shipped without `deregister()`, so crash+respawn cycles leaked routing entries. This task closes that by mandating both methods in the same task. The `ManagedWorker::run()` (P8-E3) will call `deregister()` on every exit path, but that wiring is a later task's scope.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | tokio   | 1.52.3          | rust-docs MCP  | sync (added to existing process feature) |

The existing `Cargo.toml` declares `tokio = { version = "1.52.3", features = ["process"] }`. Adding `sync` to the features array is the only dependency change. The `sync` feature was confirmed to exist in tokio 1.52.3 via the MCP `get_crate_features` tool. No new crates are introduced.

## Approach

1. **Add `sync` feature to `Cargo.toml`.** Modify `crates/anvilml-worker/Cargo.toml`: change the tokio dependency line from `features = ["process"]` to `features = ["process", "sync"]`. This enables `tokio::sync::mpsc::Sender` needed by the demux.

2. **Create `crates/anvilml-worker/src/demux.rs`.** Implement the `Demux` struct:
   - **Fields**: `inner: std::sync::Mutex<std::collections::HashMap<String, tokio::sync::mpsc::Sender<WorkerEvent>>>`.
   - **`Demux::new() -> Self`**: Returns `Self { inner: Mutex::new(HashMap::new()) }`.
   - **`Demux::register(&self, worker_id: String, tx: tokio::sync::mpsc::Sender<WorkerEvent>)`**: Locks the mutex, inserts or overwrites the entry. Returns `()`. This is idempotent — if a worker with the same ID is re-registered (e.g. after respawn), the old sender is replaced by the new one. The old sender's channel will eventually drain and close, which is safe.
   - **`Demux::deregister(&self, worker_id: &str) -> bool`**: Locks the mutex, calls `HashMap::remove()`, returns the boolean result. Safe to call on absent entries (returns `false`).
   - **`Demux::route(&self, worker_id: &str, event: WorkerEvent) -> Result<(), AnvilError>`**: Locks the mutex, looks up the worker_id in the HashMap. If found, clones the `Sender` (cheap clone — just an Arc increment), unlocks the mutex, then calls `tx.send(event).await`. If the send fails (receiver dropped), returns `Err(AnvilError::Ipc(format!("send failed for worker {worker_id}")))`. If not found, returns `Err(AnvilError::WorkerNotFound(worker_id.into()))`. The route method is `async` because it awaits on the channel send.

3. **Update `crates/anvilml-worker/src/lib.rs`.** Add two lines after the existing module declarations:
   ```rust
   mod demux;
   pub use demux::Demux;
   ```
   This keeps lib.rs well under the 80-line hard cap (~14 lines total).

4. **Create `crates/anvilml-worker/tests/demux_tests.rs`.** Write ≥5 integration tests:
   - `test_register_and_route_delivers`: Register a worker, route an event, verify the receiver gets it.
   - `test_route_worker_not_found`: Route to an unregistered worker, verify `AnvilError::WorkerNotFound` is returned.
   - `test_deregister_removes_entry` (mandatory per §9.4): Register a worker, route successfully, deregister, then route again — verify `AnvilError::WorkerNotFound`. This proves the entry was actually removed.
   - `test_double_deregister_is_safe`: Deregister an existing entry, then deregister the same ID again — verify the second call returns `false` and does not panic.
   - `test_register_overwrites`: Register a worker with sender A, then register the same worker ID with sender B, route an event, verify it arrives on B's receiver (not A's). This tests the idempotent overwrite behavior.

5. **Verify compilation and tests.** Run `cargo test -p anvilml-worker --test demux_tests` and confirm exit 0.

## Public API Surface

| Item | Crate/Module | Signature |
|------|-------------|-----------|
| `Demux::new()` | `anvilml_worker::Demux` | `pub fn new() -> Self` |
| `Demux::register()` | `anvilml_worker::Demux` | `pub fn register(&self, worker_id: String, tx: tokio::sync::mpsc::Sender<WorkerEvent>)` |
| `Demux::deregister()` | `anvilml_worker::Demux` | `pub fn deregister(&self, worker_id: &str) -> bool` |
| `Demux::route()` | `anvilml_worker::Demux` | `pub async fn route(&self, worker_id: &str, event: WorkerEvent) -> Result<(), AnvilError>` |
| `Demux` (re-export) | `anvilml_worker` | `pub use demux::Demux;` in `lib.rs` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-worker/src/demux.rs` | Demux struct with register/deregister/route |
| MODIFY | `crates/anvilml-worker/src/lib.rs` | Add `mod demux; pub use demux::Demux;` |
| MODIFY | `crates/anvilml-worker/Cargo.toml` | Add `sync` feature to tokio dependency |
| CREATE | `crates/anvilml-worker/tests/demux_tests.rs` | ≥5 integration tests for Demux |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `tests/demux_tests.rs` | `test_register_and_route_delivers` | register + route delivers event to correct receiver | None | worker_id="w1", event=WorkerEvent::Pong{seq:1} | Receiver gets the exact Pong event | `cargo test -p anvilml-worker --test demux_tests test_register_and_route_delivers` exits 0 |
| `tests/demux_tests.rs` | `test_route_worker_not_found` | route to unregistered worker returns WorkerNotFound | None | worker_id="nonexistent", event=WorkerEvent::Pong{seq:1} | Err(AnvilError::WorkerNotFound("nonexistent")) | `cargo test -p anvilml-worker --test demux_tests test_route_worker_not_found` exits 0 |
| `tests/demux_tests.rs` | `test_deregister_removes_entry` | register, deregister, then route fails — mandatory §9.4 deregistration test | Register w1 with sender A | worker_id="w1", route succeeds, deregister("w1"), route again | Second route returns Err(WorkerNotFound) | `cargo test -p anvilml-worker --test demux_tests test_deregister_removes_entry` exits 0 |
| `tests/demux_tests.rs` | `test_double_deregister_is_safe` | deregister twice on same ID is safe, second returns false | None | worker_id="w1", deregister twice | First returns true, second returns false, no panic | `cargo test -p anvilml-worker --test demux_tests test_double_deregister_is_safe` exits 0 |
| `tests/demux_tests.rs` | `test_register_overwrites` | registering same worker_id replaces old sender | None | worker_id="w1" with sender A, then w1 with sender B, route event | Event arrives on B's receiver, not A's | `cargo test -p anvilml-worker --test demux_tests test_register_overwrites` exits 0 |

## CI Impact

No CI changes required. The new test file `tests/demux_tests.rs` is a standard Rust integration test crate under `crates/anvilml-worker/tests/`. The existing CI job `rust-linux` (which runs `cargo test --workspace --features mock-hardware`) automatically picks up all test crates in the workspace. No new CI jobs, gates, or configurations are needed.

## Platform Considerations

None identified. The `Demux` struct uses only `std::sync::Mutex` (cross-platform), `std::collections::HashMap` (cross-platform), and `tokio::sync::mpsc` (cross-platform). No `#[cfg(unix)]` or `#[cfg(windows)]` guards are required. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `tokio::sync::mpsc::Sender::send()` is async and returns `Result<(), SendError<T>>` — the route method must handle the case where the receiver has been dropped (worker died). The error message must include the worker_id for diagnostics. | Medium | Medium | The approach uses `tx.send(event).await.map_err(|e| AnvilError::Ipc(...))` which surfaces the dropped-receiver case as an `Ipc` error with context. The test suite doesn't test this edge case explicitly (it's exercised by later tasks that spawn/kill workers), but the error path is straightforward and tested implicitly by the route test. |
| The `sync` feature addition to tokio changes the dependency graph — it pulls in `parking_lot` as a transitive dependency, which could affect build times or binary size. | Low | Low | `parking_lot` is already pulled in by other crates via `tokio`'s `full` feature in workspace dependencies. Adding `sync` to just `anvilml-worker`'s tokio feature list does not change the workspace-level dependency resolution. Verified by running `cargo tree -p anvilml-worker` after the change. |
| The `route()` method holds the mutex lock while awaiting `tx.send()`. If the channel is full, this blocks the lock for potentially long periods, starving other register/deregister calls. | Low | Medium | For the MVP scope, this is acceptable: the demux is only called from the bridge reader task (one caller), and register/deregister are infrequent (once per spawn, once per exit). The design doc §9.4 does not specify a non-blocking send. If this becomes a bottleneck in practice, it would be addressed in a dedicated perf task, not this one. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --test demux_tests` exits 0
- [ ] `cargo check -p anvilml-worker` exits 0 (compilation check)
- [ ] `wc -l crates/anvilml-worker/src/lib.rs` shows ≤ 80 lines
- [ ] `grep -c "^#\[test\]" crates/anvilml-worker/tests/demux_tests.rs` shows ≥ 5 test functions
- [ ] `grep "deregister" crates/anvilml-worker/tests/demux_tests.rs` confirms a deregistration test exists
