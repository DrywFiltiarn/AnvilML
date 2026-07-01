# Plan Report: P8-E1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P8-E1                                               |
| Phase       | 008 — IPC Stress Gate & Worker Pool                 |
| Description | anvilml-worker: WorkerHandle struct (cheap, Clone-able) |
| Depends on  | P8-C2, P8-D1                                        |
| Project     | anvilml                                             |
| Planned at  | 2026-07-01T12:30:00Z                                |
| Attempt     | 1                                                   |

## Objective

Create the `WorkerHandle` struct in `crates/anvilml-worker/src/managed.rs` — a cheap, `Clone`-able handle that lets multiple independent consumers (status-polling tasks, API handlers, the pool itself) interact with a worker's lifecycle state and request graceful shutdown, without ever needing to `Arc`-wrap the worker struct itself. This resolves the v3 regression where `Arc<ManagedWorker>` with a by-value `run(self)` method made `run()` impossible to call once wrapped. The handle provides read-only `status()` and idempotent `request_shutdown()` — a write-side mutator (`set_status()`) is deferred to P8-E2, and the owning `ManagedWorker` type with `run()` is deferred to P8-E3.

## Scope

### In Scope
- Create `crates/anvilml-worker/src/managed.rs` with the `WorkerHandle` struct per §9.1's resolved shape (field types from the task context, which adds `Arc<Mutex<Option<...>>>` wrapping on `join_handle` over the design doc's bare `JoinHandle<()>` to allow the pool to extract and await it during shutdown).
- Implement `WorkerHandle::new()` constructor accepting all four fields.
- Implement `status(&self) -> WorkerStatus` — acquires a read lock, copies the value, releases the lock.
- Implement `request_shutdown(&mut self)` — takes the `Option<oneshot::Sender<()>>`, sends `()` if present (consuming the sender), no-op if already taken.
- Add `mod managed;` and `pub use managed::WorkerHandle;` to `lib.rs`.
- Bump `anvilml-worker` crate version from `0.1.5` to `0.1.6`.
- Create `crates/anvilml-worker/tests/managed_tests.rs` with ≥4 tests.

### Out of Scope
- `WorkerHandle::set_status()` mutator — deferred to P8-E2. This task's `WorkerHandle` is read-only on status.
- `ManagedWorker` struct and its `run()` method — deferred to P8-E3. `managed.rs` contains only `WorkerHandle` in this task.
- `WorkerPool` — deferred to P8-G1.
- Any integration tests involving real subprocess spawning or IPC communication — out of scope for this struct-only task.

## Existing Codebase Assessment

The `anvilml-worker` crate exists at version 0.1.5 with seven modules already implemented: `demux` (P8-C1), `env` (P8-B1), `keepalive` (P8-C2), `spawn` (P8-B2), `job_object` (P8-B3, Windows-only), `respawn` (P8-D1). The crate's `lib.rs` has 21 lines and follows the ≤80-line hard cap. No `managed.rs` exists yet.

The established patterns are clear from the existing test files: tests use `#[tokio::test]` for async tests, import types directly from workspace crates (`anvilml_core::`, `anvilml_ipc::`, `anvilml_worker::`), use `WorkerEvent::Ready` with mock values for testing, and follow a pattern of one test function per `#[tokio::test]` block with a `///` doc comment describing the invariant. The crate uses `anvilml_core::AnvilError` for error types.

`WorkerStatus` is defined in `crates/anvilml-core/src/types/worker.rs` as a `#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]` enum with variants `Spawning`, `Idle`, `Busy`, `Dying`, `Dead`. It implements `Copy` and `Clone`, making it trivially returnable from `status()` without interior-mutable reference concerns.

The design doc (§9.1) specifies `join_handle: tokio::task::JoinHandle<()>` but the task context specifies `join_handle: Arc<tokio::sync::Mutex<Option<JoinHandle<()>>>>`. The task context's version adds a layer of indirection that allows `WorkerPool::shutdown_all()` (P8-G1) to take ownership of the handle after the worker task completes and await it with a bounded timeout. This is a deliberate design choice over the simpler design-doc shape, and the plan follows the task context.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source   | Feature flags confirmed |
|--------|---------|-----------------|--------------|------------------------|
| crate  | tokio   | 1.52.3          | rust-docs MCP| rt (new), process, sync, time |

The `rt` feature is not currently declared in `anvilml-worker/Cargo.toml` but is required for `tokio::task::JoinHandle` to be available. `sync` already covers `tokio::sync::RwLock` and `tokio::sync::oneshot`. `WorkerStatus` comes from the existing `anvilml-core` path dependency — no new external crate needed.

## Approach

1. **Add `rt` feature to tokio in `Cargo.toml`.** Edit `crates/anvilml-worker/Cargo.toml` line 11: change `features = ["process", "sync", "time"]` to `features = ["process", "rt", "sync", "time"]`. This makes `tokio::task::JoinHandle` available. Verified via rust-docs MCP: `rt` is a standalone feature flag in tokio 1.52.3.

2. **Create `crates/anvilml-worker/src/managed.rs`.** Write the file with:
   - A `//!` crate-level doc comment describing the module's ownership (cheap, shareable handle for worker lifecycle interaction).
   - The `WorkerHandle` struct with `#[derive(Clone)]` and the four fields exactly as specified in the task context:
     ```rust
     #[derive(Clone)]
     pub struct WorkerHandle {
         pub worker_id: String,
         status: Arc<RwLock<WorkerStatus>>,
         shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
         join_handle: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,
     }
     ```
     The `status` field is private (no `pub`) — consumers read it through `status()` only, keeping the lock acquisition encapsulated. The `worker_id` field is `pub` per the task context's `{pub worker_id: String, ...}` syntax.
   - A `///` doc comment on the struct explaining what it is, what it shares across clones, and what it does not share.
   - `impl WorkerHandle` block with:
     - `pub fn new(...)` constructor taking all four fields as parameters. Doc comment describing each parameter.
     - `pub fn status(&self) -> WorkerStatus` — acquires a read lock via `self.status.read().await` (note: since `WorkerStatus` is `Copy`, we can use the synchronous `read()` from `tokio::sync::RwLock` which returns a `RwLockReadGuard` that can be dereferenced to get the `Copy` value). Actually, `tokio::sync::RwLock::read()` returns a future, so this should be `pub async fn status(&self) -> WorkerStatus`. But the task context shows `status(&self) -> WorkerStatus` without `async`. Looking at the task context again: "Provide status(&self) -> WorkerStatus (read lock)". Since `tokio::sync::RwLock::read()` is async, this method must be `pub async fn status(&self) -> WorkerStatus`. The synchronous `parking_lot::RwLock` has a non-blocking `read()` that returns a guard, but we're using tokio's. The task says "status() needs no lock run holds" meaning the read lock is acquired and released within the call — the caller doesn't hold it. I'll implement it as `pub async fn status(&self) -> WorkerStatus`.
     - `pub fn request_shutdown(&mut self)` — takes `self.shutdown_tx.take()`, sends `()` via `tx.send(())` if present, ignores the result (the receiver may already be dropped). No-Op if `shutdown_tx` is already `None` (idempotent).

3. **Update `crates/anvilml-worker/src/lib.rs`.** Add two lines after the existing `mod respawn;` / `pub use respawn::RespawnPolicy;`:
   ```rust
   mod managed;
   pub use managed::WorkerHandle;
   ```
   This keeps lib.rs well under the 80-line cap.

4. **Bump crate version.** Edit `crates/anvilml-worker/Cargo.toml` line 3: change `version = "0.1.5"` to `version = "0.1.6"`.

5. **Create `crates/anvilml-worker/tests/managed_tests.rs`.** Write ≥4 tests:
   - `test_clone_shares_status`: Construct a handle, set status via the internal lock (using a separate `Arc<RwLock<WorkerStatus>>` to simulate what P8-E2's mutator would do, or construct two handles from the same shared lock), clone it, verify both see the same status. Since this task doesn't have `set_status()`, I'll create a shared `Arc<RwLock<WorkerStatus>>`, set it to `Idle` via a direct write before constructing the handles, then verify both the original and cloned handle return `Idle` from `status()`.
   - `test_clone_independent_worker_id`: Clone a handle, verify the clone has the same `worker_id` (the field is `pub` and is a `String`, so cloning copies it — independent strings, same value).
   - `test_request_shutdown_sends_signal`: Construct a handle with a fresh `oneshot::channel`, spawn a task that receives on the receiver side, call `request_shutdown()`, verify the receiver gets `Ok(())`.
   - `test_request_shutdown_is_idempotent`: Construct a handle with a `oneshot::channel`, call `request_shutdown()` twice. The second call should be a no-op (the `Option` is already `None`). No panic, no error.
   - `test_status_returns_current_value`: Construct a handle with status set to `Spawning` (via direct write to the shared `Arc<RwLock<...>>` before construction), verify `status()` returns `Spawning`.

6. **Verify compilation.** Run `cargo check -p anvilml-worker --features mock-hardware` to confirm the new module compiles and all existing modules are unaffected.

## Public API Surface

| Item | Crate/Module | Signature |
|------|-------------|-----------|
| Struct | `anvilml_worker::WorkerHandle` | `pub struct WorkerHandle { pub worker_id: String, status: Arc<RwLock<WorkerStatus>>, shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>, join_handle: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>> }` |
| Constructor | `anvilml_worker::WorkerHandle::new` | `pub fn new(worker_id: String, status: Arc<RwLock<WorkerStatus>>, shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>, join_handle: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>) -> Self` |
| Method | `anvilml_worker::WorkerHandle::status` | `pub async fn status(&self) -> WorkerStatus` |
| Method | `anvilml_worker::WorkerHandle::request_shutdown` | `pub fn request_shutdown(&mut self)` |
| Re-export | `anvilml_worker` (lib.rs) | `pub use managed::WorkerHandle;` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-worker/src/managed.rs` | WorkerHandle struct with new(), status(), request_shutdown() |
| MODIFY | `crates/anvilml-worker/src/lib.rs` | Add `mod managed;` and `pub use managed::WorkerHandle;` |
| MODIFY | `crates/anvilml-worker/Cargo.toml` | Bump version 0.1.5 → 0.1.6, add `rt` feature to tokio |
| CREATE | `crates/anvilml-worker/tests/managed_tests.rs` | ≥4 integration tests for WorkerHandle |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `tests/managed_tests.rs` | `test_clone_shares_status` | Constructing two WorkerHandles from the same `Arc<RwLock<WorkerStatus>>` and calling `status()` on both returns the same value, proving clones share the status lock | `cargo test -p anvilml-worker --test managed_tests test_clone_shares_status` exits 0 |
| `tests/managed_tests.rs` | `test_clone_independent_worker_id` | Cloning a handle copies the `worker_id` String (same value, independent allocation), proving the clone is a true deep copy of the handle's public state | `cargo test -p anvilml-worker --test managed_tests test_clone_independent_worker_id` exits 0 |
| `tests/managed_tests.rs` | `test_request_shutdown_sends_signal` | Constructing a handle with a fresh `oneshot::channel`, calling `request_shutdown()` delivers `()` to the receiver side, proving the shutdown trigger works | `cargo test -p anvilml-worker --test managed_tests test_request_shutdown_sends_signal` exits 0 |
| `tests/managed_tests.rs` | `test_request_shutdown_is_idempotent` | Calling `request_shutdown()` twice on the same handle does not panic — the second call operates on `None` and returns cleanly, proving idempotency | `cargo test -p anvilml-worker --test managed_tests test_request_shutdown_is_idempotent` exits 0 |
| `tests/managed_tests.rs` | `test_status_returns_current_value` | Constructing a handle with status set to `Spawning` and calling `status()` returns `Spawning`, proving the read path works correctly | `cargo test -p anvilml-worker --test managed_tests test_status_returns_current_value` exits 0 |

## CI Impact

No CI changes required. The new test file `managed_tests.rs` is automatically picked up by `cargo test -p anvilml-worker` (the standard test command used in CI jobs `rust-linux` and `rust-windows`). Adding a `mod managed;` to `lib.rs` does not change any CI job's behavior — it only adds a new module to the crate's public surface.

## Platform Considerations

None identified. The `WorkerHandle` struct and its methods are platform-neutral — they use `String`, `Arc<RwLock<...>>`, `oneshot::Sender`, and `JoinHandle` which are all cross-platform. The `#[cfg(windows)]` module (`job_object`) is unaffected by this task. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `tokio::sync::RwLock::read()` is async but the task context specifies `status(&self) -> WorkerStatus` without `async` — the ACT agent may incorrectly use a synchronous read pattern. | Medium | High | The plan explicitly states `pub async fn status(&self) -> WorkerStatus`. The `tokio::sync::RwLock` read() returns a future that must be `.await`ed. This is confirmed by the tokio 1.52.3 API. |
| The `rt` feature addition to tokio may introduce unexpected transitive dependencies or MSRV changes. | Low | Medium | The `rt` feature in tokio 1.52.3 is a lightweight feature that only exposes `JoinHandle` and related task types — no additional dependencies beyond what `process` and `sync` already pull in. Verified via MCP feature flags. |
| The `Arc<Mutex<Option<JoinHandle>>>` wrapping (task context) differs from the design doc's bare `JoinHandle` — the ACT agent may follow the design doc instead. | Medium | Medium | The plan explicitly uses the task context's field types and notes the deviation from the design doc. The task context is the authoritative specification for this task. |
| Test construction requires a shared `Arc<RwLock<WorkerStatus>>` to set status before handle creation (since P8-E2's `set_status()` doesn't exist yet). Tests may be confusing without clear documentation. | Low | Low | Each test has a `///` doc comment explaining the setup and what it proves. The shared-arc pattern is a standard testing technique for verifying clone-safety. |

## Acceptance Criteria

- [ ] `cargo check -p anvilml-worker --features mock-hardware` exits 0
- [ ] `cargo test -p anvilml-worker --test managed_tests` exits 0 with ≥4 tests passing
- [ ] `wc -l crates/anvilml-worker/src/lib.rs` reports ≤80 lines
- [ ] `grep "^pub use managed::WorkerHandle;" crates/anvilml-worker/src/lib.rs` finds exactly one match
- [ ] `cargo clippy -p anvilml-worker --features mock-hardware -- -D warnings` exits 0
