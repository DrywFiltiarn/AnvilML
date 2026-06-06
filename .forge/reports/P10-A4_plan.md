# Plan Report: P10-A4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P10-A4                                        |
| Phase       | 010 — Worker Crash Recovery                   |
| Description | anvilml: test-only worker PID accessor for crash-recovery proof |
| Depends on  | P10-A3                                        |
| Project     | anvilml                                       |
| Planned at  | 2026-06-06T17:40:00Z                          |
| Attempt     | 1                                             |

## Objective

Add a test-only accessor `pid_for(&self, worker_id: &str) -> Option<u32>` on `WorkerPool` in `anvilml-worker/src/pool.rs`, gated behind `#[cfg(any(test, feature = "test-helpers"))]`. This enables downstream integration tests and the Runnable Proof (kill a worker PID, observe Dead→Respawning→Idle) by allowing external code to discover the child process PID for any named worker.

## Scope

### In Scope
1. Add `test-helpers` feature flag to `anvilml-worker/Cargo.toml`.
2. Implement `pid_for(&self, worker_id: &str) -> Option<u32>` on `WorkerPool`, gated with `#[cfg(any(test, feature = "test-helpers"))]`. The method iterates workers by `worker_id()`, acquires the child lock, and returns `child.id()` (the `tokio::process::Child` PID accessor that returns `Option<u32>`).
3. Add a unit test in `pool.rs` verifying `pid_for` returns `Some(pid)` for an existing worker ID and `None` for a non-existent one (using the manually-constructed pool pattern already used in existing tests).

### Out of Scope
- No changes to `managed.rs`, `lib.rs`, or any other crate.
- No production runtime code changes — this is purely test-instrumentation.
- No server handler, WebSocket, or API changes.
- No version bump (no source files outside `pool.rs` are modified; `Cargo.toml` only adds a feature flag, not a version field).

## Approach

1. **Add feature flag** — In `anvilml-worker/Cargo.toml`, add `test-helpers = []` to the `[features]` section. This is a leaf feature with no dependency forwarding (unlike `mock-hardware`).

2. **Implement `pid_for` on `WorkerPool`** — Append the method after the existing public methods in `impl WorkerPool`. The implementation:
   ```rust
   #[cfg(any(test, feature = "test-helpers"))]
   impl WorkerPool {
       /// Return the child process PID for the worker with the given ID.
       ///
       /// Returns `None` if no worker matches or if the child has not been spawned
       /// yet (or has already exited). This is a test-only accessor gated behind
       /// `#[cfg(any(test, feature = "test-helpers"))]`.
       pub async fn pid_for(&self, worker_id: &str) -> Option<u32> {
           for worker in &self.workers {
               if worker.worker_id() == worker_id {
                   let child = worker.child.lock().await;
                   return child.as_ref().and_then(|c| c.id());
               }
           }
           None
       }
   }
   ```
   This uses a separate `#[cfg(...)] impl` block so the method is entirely absent from non-test, non-`test-helpers` builds. It follows the existing pattern of iterating by `worker_id()` (identical to `set_busy`, `set_idle`, and `send`).

3. **Add unit test** — Append a test in `mod tests` within `pool.rs`:
   - Constructs a pool manually (same pattern as existing `spawn_all_creates_cpu_worker_when_no_gpus` test).
   - Calls `pid_for("worker-0")` and asserts it returns `None` when no child is stored.
   - Sets a dummy child via `worker.child.lock().await` to verify the accessor path (or uses `spawn_dummy_child()` from `managed.rs` tests if accessible; alternatively, directly sets the `child` field in the manually-constructed pool).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/Cargo.toml` | Add `test-helpers = []` feature flag |
| Modify | `crates/anvilml-worker/src/pool.rs` | Add `pid_for` method + unit test, both cfg-gated |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-worker/src/pool.rs` (mod tests) | `pid_for_returns_none_for_missing_worker` | `pid_for("nonexistent")` returns `None` on a pool with no matching worker |
| `crates/anvilml-worker/src/pool.rs` (mod tests) | `pid_for_returns_child_pid_when_spawned` | After setting a child on the worker, `pid_for("worker-0")` returns `Some(pid)` matching `child.id()` |

## CI Impact

No CI workflow files are modified. The new feature flag is opt-in and does not affect existing gates. The test suite (`cargo test --workspace --features mock-hardware`) must exit 0, and the Windows cross-check (`cargo check --target x86_64-pc-windows-gnu --features mock-hardware`) must also pass. Since `pid_for` is cfg-gated behind `test` or `test-helpers`, it compiles cleanly under both native Linux and cross-compiled Windows targets.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `child.id()` returns `None` after child exits | High | Test may fail to assert a specific PID value | Assert `is_some()` rather than exact value; verify it matches the stored child's `id()` within the same test scope |
| Feature flag forwarding not needed for `test-helpers` (unlike `mock-hardware`) | Low | Downstream crates can't use the accessor | Document that `test-helpers` is anvilml-worker-local only; later tasks that need it will add their own cfg-gated imports |
| Separate `#[cfg] impl` block causes clippy warnings about split impls | Medium | Clippy may warn about split impl blocks | Suppress with `#[allow(clippy::needless_pass_by_ref_mut)]` or use inline methods in the same impl if clippy complains; this is a known Rust pattern for cfg-gated methods |

## Acceptance Criteria

- [ ] `crates/anvilml-worker/Cargo.toml` contains `test-helpers = []` in `[features]`
- [ ] `WorkerPool::pid_for(&self, worker_id: &str) -> Option<u32>` exists in `pool.rs`, gated with `#[cfg(any(test, feature = "test-helpers"))]`
- [ ] `cargo test --workspace --features mock-hardware` exits 0
- [ ] `cargo check --target x86_64-pc-windows-gnu --features mock-hardware` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
