# Plan Report: P11-A2

| Field       | Value                                                              |
|-------------|---------------------------------------------------------------------|
| Task ID     | P11-A2                                                              |
| Phase       | 011 — Dynamic Node Registry                                         |
| Description | anvilml-worker: on Ready event update NodeTypeRegistry in scheduler |
| Depends on  | P11-A1                                                              |
| Project     | anvilml                                                             |
| Planned at  | 2026-06-19T08:40:00Z                                                |
| Attempt     | 1                                                                   |

## Objective

Wire `NodeTypeRegistry` (created in P11-A1) into the `WorkerPool` / `ManagedWorker` event loop so that when a worker emits `WorkerEvent::Ready`, its `node_types` field is forwarded to `NodeTypeRegistry::update_from_worker()`. This closes the loop between P11-A1 (registry creation) and the worker event system, enabling the scheduler to know which node types are available at runtime.

## Scope

### In Scope
- Add `node_registry: Option<Arc<NodeTypeRegistry>>` field to `ManagedWorker` in `managed.rs`.
- Extend `ManagedWorker::spawn()` to accept `Arc<NodeTypeRegistry>` and store it.
- Extend `ManagedWorker::new()` to accept `Option<Arc<NodeTypeRegistry>>` (test constructor).
- Extend `do_respawn` to forward the same `Arc<NodeTypeRegistry>` into the recursive `Self::spawn()` call, guarded the same way `routes` already is.
- In `ManagedWorker::run()`'s `Ready` match arm, destructure `node_types` (plus `torch_version`/`fp8`, needed for the log fix below) and call `node_registry.update_from_worker(&self.worker_id, node_types.clone())`.
- Add `node_registry: Arc<NodeTypeRegistry>` parameter to `WorkerPool::spawn_all()`; forward it into each `ManagedWorker::spawn()` call.
- Wire the registry construction into `backend/src/main.rs` ahead of the `spawn_all()` call.
- Bump `anvilml-worker` patch version.
- Add a test verifying the registry-update wiring.

### Out of Scope
- Any change to `NodeTypeRegistry`'s own public method signatures from what P11-A1 left (other than the deviation logged below, discovered mid-implementation — see Deviations).
- `crates/anvilml-server`'s `AppState`, `GET /v1/nodes`, or any `anvilml-server/src/state.rs` change (P11-A3).
- Any change to the Python worker side (P11-B1).
- Any graph validation logic (P11-P12).
- `WorkerPool::new()` (the test constructor) — confirmed during the codebase assessment below to never construct a `ManagedWorker`, so it needs no `node_registry` parameter.

## Existing Codebase Assessment

P11-A1 created `NodeTypeRegistry` in `crates/anvilml-scheduler/src/node_registry.rs` with the full public API: `new()`, `update_from_worker(worker_id: &str, types: Vec<NodeTypeDescriptor>)`, `get()`, `all_types()`, `is_empty()`. It is re-exported from `anvilml-scheduler`'s `lib.rs`.

The `WorkerEvent::Ready` variant (`anvilml-ipc/src/messages.rs`) carries `worker_id`, `device_index`, `device_name`, `device_type`, `vram_total_mib`, `vram_free_mib`, `torch_version`, `fp16`, `bf16`, `fp8`, `flash_attention`, and `node_types: Vec<NodeTypeDescriptor>`. `ManagedWorker::run()`'s existing `Ready` arm destructures only `device_name`/`device_index` and absorbs the rest with `..` — `node_types`, `torch_version`, and `fp8` are all currently discarded.

`ManagedWorker::new()`'s current signature has 18 parameters, ending in `ready_tx: Option<oneshot::Sender<()>>`. `ManagedWorker::spawn()` has 6 parameters, ending in `restart_rx: tokio::sync::watch::Receiver<u64>`. `do_respawn` already has an established guard pattern for optional fields that production workers always have but test workers (built via `new()`) may not: `routes` is checked with a `match self.routes.clone() { Some(r) => r, None => return Err(...) }` immediately before the recursive `Self::spawn()` call, rather than `.expect()`/panicking.

**Dependency-cycle finding (drives the central design decision of this plan):** `anvilml-scheduler`'s `Cargo.toml` already depends on `anvilml-worker` (for the future dispatch loop — `anvilml-scheduler`'s crate doc describes its eventual role as owning "the dispatch loop"). If `anvilml-worker` needs to call a method on a type that lives in `anvilml-scheduler` (`NodeTypeRegistry::update_from_worker`), the only way to do so is for `anvilml-worker` to depend on `anvilml-scheduler` — which would create a cycle: `anvilml-scheduler → anvilml-worker → anvilml-scheduler`. Two resolutions were considered:

1. Remove the (currently unused — confirmed via `grep -rn anvilml_worker crates/anvilml-scheduler/src/` returning no matches) `anvilml-worker` dependency from `anvilml-scheduler`. Rejected: this only defers the cycle to whichever future task implements the scheduler's dispatch loop, at which point the same cycle reappears in a more confusing form (a previously-working build breaking when an unrelated task adds one `use` statement).
2. Move `NodeTypeRegistry` to `anvilml-core`, which both `anvilml-worker` and `anvilml-scheduler` already depend on directly. This breaks the cycle permanently rather than deferring it.

Option 2 is adopted. `anvilml-core`'s crate-level doc comment currently states "Hard constraints: Zero I/O. Zero async. Zero network" — `NodeTypeRegistry` is `Arc<RwLock<...>>`-backed with `async fn` methods, so this move requires an explicit, narrow, documented exception to that constraint, not a silent violation. The plan amends `anvilml-core`'s crate doc to state the exception and the reason for it.

`WorkerPool::spawn_all()` currently accepts `cfg`, `devices`, `transport`, `broadcaster`. It spawns each `ManagedWorker` via `ManagedWorker::spawn()` and immediately spawns `worker.run()` as its own detached task — `WorkerPool` never retains a `ManagedWorker` instance afterward, only a `WorkerHandle` (status `Arc`, shutdown/restart senders, join handle, worker_id, device_name). `WorkerPool::new()` (the test constructor) takes pre-built `(status, worker_id, device_name)` triples and never touches `ManagedWorker` at all — confirmed by inspection of every call site in `pool_tests.rs`, `stats_tick_tests.rs`, and `workers_tests.rs`. This means `WorkerPool` itself does not need a `node_registry` struct field: `spawn_all()` only needs to thread the caller's `Arc` through to each `ManagedWorker::spawn()` call. P11-A3's own task description (`docs/TASKS_PHASE011.md`) gives `AppState` its own independent `Arc<NodeTypeRegistry>` field, not one sourced through `WorkerPool` — confirming a pool-level field would likely go permanently unread.

`ENVIRONMENT.md §9`'s mandatory INFO log table requires the "Worker reached Ready" log point to carry `worker_id=`, `device=`, `torch_version=`, `fp8=`, `node_count=` — five fields. The existing log call in `managed.rs` only carries `worker_id=` and `device=`. This gap predates P11-A2 but this task touches the exact code surrounding that log call, so per `ENVIRONMENT.md §9`'s rule ("Every task that touches the relevant subsystem must verify these log calls exist... A task is not complete if a mandatory INFO log point is absent"), fixing it is in scope for this task even though it isn't a registry-specific change.

## Resolved Dependencies

| Type   | Name              | Version verified | MCP source              | Feature flags confirmed |
|--------|-------------------|-------------------|--------------------------|--------------------------|
| crate  | anvilml-scheduler | 0.1.1             | Cargo.toml (workspace path dep) | mock-hardware |
| crate  | anvilml-core      | 0.1.13            | Cargo.toml (workspace path dep) | n/a |
| crate  | hashbrown         | 0.17               | Already resolved in P11-A1 (Cargo.lock) | n/a |
| crate  | tokio             | 1.52.3             | Workspace                | full |

No new external crates are introduced. `hashbrown` and `tokio` move from being dependencies of `anvilml-scheduler` to being dependencies of `anvilml-core` (where `NodeTypeRegistry` is relocating to), at the same already-resolved versions — no version change, only a dependency-graph relocation.

## Approach

1. **Move `NodeTypeRegistry` from `anvilml-scheduler` to `anvilml-core`.** Create `crates/anvilml-core/src/node_registry.rs` with the struct and all methods unchanged from P11-A1, except the import path (`crate::types::NodeTypeDescriptor` instead of `anvilml_core::NodeTypeDescriptor`, since the module now lives inside `anvilml-core` itself). Add a module-doc section explaining the relocation and the `anvilml-core` "zero async" exception, so a future reader doesn't need to re-derive why an async, lock-backed type lives in a "pure data" crate. Delete `crates/anvilml-scheduler/src/node_registry.rs`. `anvilml-core/Cargo.toml` gains `hashbrown` and `tokio` (the latter already present as a workspace dependency, just not yet listed for this crate). `anvilml-scheduler/Cargo.toml` loses `hashbrown` (no longer needed once the registry moves out). `anvilml-scheduler/src/lib.rs`'s `pub mod node_registry; pub use node_registry::NodeTypeRegistry;` becomes `pub use anvilml_core::NodeTypeRegistry;` — a re-export, preserving the import path `anvilml_scheduler::NodeTypeRegistry` for `crates/anvilml-scheduler/tests/node_registry_tests.rs`, which already imports it that way and should not need modification for this reason.

2. **Amend `anvilml-core`'s crate-level doc comment.** Change "Zero I/O. Zero async. Zero network." to acknowledge the one narrow exception this task introduces, naming `node_registry` specifically and stating that no other module should follow its example.

3. **Add `node_registry` field to `ManagedWorker`.** `Option<Arc<NodeTypeRegistry>>`, following the same `Option`-for-test-compatibility pattern already used for `routes`, `route_key`, and `ready_tx`.

4. **Extend `ManagedWorker::new()`** with a `node_registry: Option<Arc<NodeTypeRegistry>>` parameter (19th positional parameter, after `ready_tx`), stored directly.

5. **Extend `ManagedWorker::spawn()`** with a `node_registry: Arc<NodeTypeRegistry>` parameter, stored as `Some(node_registry)` in the constructed `Self`.

6. **Extend `do_respawn`** to forward the registry into the recursive `Self::spawn()` call. Add a guard mirroring the existing `routes` guard exactly — `match self.node_registry.clone() { Some(r) => r, None => return Err(AnvilError::Internal(...)) }` — rather than `.expect()`/panicking, consistent with how this exact function already treats `routes` as an optional-in-tests, mandatory-in-production dependency.

7. **In `run()`'s `Ready` match arm**, destructure `torch_version`, `fp8`, `node_types` in addition to the existing `device_name`/`device_index`. Call `node_registry.update_from_worker(&self.worker_id, node_types.clone())` unconditionally for every `Ready` event — not gated on the `Initializing → Idle` transition below it — since node types are a property of the event itself, not of that particular state transition. Fix the existing "worker reached Ready" `tracing::info!` call to include `torch_version` and `node_count` alongside the existing `worker_id`/`device`/`fp8`-missing fields, per `ENVIRONMENT.md §9`'s five-field requirement (see Existing Codebase Assessment).

8. **Extend `WorkerPool::spawn_all()`** with a `node_registry: Arc<NodeTypeRegistry>` parameter, cloned (`Arc::clone`) into each `ManagedWorker::spawn()` call inside the per-device loop. `WorkerPool::new()` is left untouched — confirmed in step 1's assessment that it never constructs a `ManagedWorker`.

9. **Wire `backend/src/main.rs`.** Construct `Arc::new(NodeTypeRegistry::new().await)` once, before the `spawn_all()` call, and pass `Arc::clone(&node_registry)` into it. Do not pass it into `AppState::new_with_hardware` — that's P11-A3's task, and `AppState`'s constructor doesn't yet accept it.

10. **Bump `anvilml-worker`'s patch version.** Also bump `anvilml-core` and `anvilml-scheduler` (both have source-file changes per this plan) and `backend` (its `main.rs` changes), per `ENVIRONMENT.md §12`'s rule that every crate with modified source files gets a patch bump.

11. **Add a test verifying the wiring**, in `crates/anvilml-worker/tests/pool_tests.rs` per the task's file-targeting. The natural approach — spawn `worker.run()`, send a real `Ready` event through the broadcast channel, assert the registry was updated — is the preferred design, but `managed_tests.rs` already has a precedent of exactly this pattern (`test_run_ready_event_releases_keepalive_gate`, which drives a real `Ready` event through `run()`'s `select!` loop and asserts on `ready_tx`'s side effect, in the same match arm `node_registry` now lives in). If the new test cannot be made to reliably reach `run()`'s loop within a reasonable debugging budget, a direct-call fallback (construct the worker with `Some(registry)`, then call `update_from_worker` directly with the same arguments `run()` would pass) is acceptable, **provided the test's own doc comment states plainly that it does not exercise `run()`'s `select!` loop and explains why** — this codebase has at least one documented precedent of `run()`/`spawn_run` test-harness footguns (a leading-underscore `_shutdown_tx` binding dropping before `run()`'s first poll, causing the loop to tear down silently before ever reaching its event arm — see `managed_tests.rs`'s `spawn_run` doc comment) that should be ruled out, or at minimum named as the likely cause, before falling back, rather than leaving an unexplained gap in test coverage.

## Public API Surface

| Item | Path | Before | After |
|------|------|--------|-------|
| module | `anvilml_core::node_registry` | does not exist | new — relocated from `anvilml_scheduler::node_registry` |
| struct | `anvilml_core::NodeTypeRegistry` | does not exist | new — relocated, same fields/methods as P11-A1 |
| re-export | `anvilml_scheduler::NodeTypeRegistry` | `pub use node_registry::NodeTypeRegistry` (local module) | `pub use anvilml_core::NodeTypeRegistry` (cross-crate re-export) |
| fn | `ManagedWorker::new()` | 18 parameters, ends `ready_tx` | 19 parameters, ends `node_registry: Option<Arc<NodeTypeRegistry>>` |
| fn | `ManagedWorker::spawn()` | 6 parameters, ends `restart_rx` | 7 parameters, ends `node_registry: Arc<NodeTypeRegistry>` |
| fn | `WorkerPool::spawn_all()` | 4 parameters, ends `broadcaster` | 5 parameters, ends `node_registry: Arc<NodeTypeRegistry>` |
| fn | `WorkerPool::new()` | 3 parameters | unchanged — never constructs a `ManagedWorker` |

No new `pub fn`, `pub struct`, `pub enum`, `pub trait`, `pub const`, or `pub type` items in `anvilml-worker` itself — only existing public signatures extended, and a struct/module relocated (with re-export) in `anvilml-core`/`anvilml-scheduler`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/node_registry.rs` | `NodeTypeRegistry`, moved from `anvilml-scheduler` |
| DELETE | `crates/anvilml-scheduler/src/node_registry.rs` | Moved to `anvilml-core` |
| MODIFY | `crates/anvilml-core/src/lib.rs` | Add `pub mod node_registry` + re-export; amend crate doc's "zero async" claim |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Add `hashbrown`, `tokio`; bump version |
| MODIFY | `crates/anvilml-scheduler/src/lib.rs` | Re-export `NodeTypeRegistry` from `anvilml_core` instead of a local module |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Remove `hashbrown`; bump version |
| MODIFY | `crates/anvilml-worker/src/managed.rs` | `node_registry` field; extend `new()`/`spawn()`/`do_respawn`; wire `run()`'s `Ready` arm; fix INFO log fields |
| MODIFY | `crates/anvilml-worker/src/pool.rs` | Extend `spawn_all()`; forward registry to each `ManagedWorker::spawn()` |
| MODIFY | `crates/anvilml-worker/Cargo.toml` | Bump version |
| MODIFY | `backend/src/main.rs` | Construct registry; pass to `spawn_all()` |
| MODIFY | `backend/Cargo.toml` | Bump version |
| MODIFY | `crates/anvilml-worker/tests/managed_tests.rs` | Add `node_registry` param to all 7 `ManagedWorker::new()` call sites |
| MODIFY | `crates/anvilml-worker/tests/pool_tests.rs` | Add `node_registry` param to `make_test_worker`; add new test |
| MODIFY | `crates/anvilml-server/tests/workers_tests.rs` | Add `node_registry` param to its one `ManagedWorker::new()` call site |
| MODIFY | `docs/TESTS.md` | Add entry for the new test |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-------------------|----------------|--------|-------------------|----------------------|
| `crates/anvilml-worker/tests/pool_tests.rs` | (new test — exact name and approach per Approach step 11) | The registry-update wiring: a `ManagedWorker` constructed with `Some(registry)` correctly forwards into `update_from_worker` | `NodeTypeRegistry::new()`, `ManagedWorker::new()` with `Some(registry)` | `NodeTypeDescriptor` values | Registry reflects the update (`get`/`all_types` correct) | `cargo test -p anvilml-worker --features mock-hardware` exits 0 |

## CI Impact

No CI changes required. All modified/new files are picked up by the existing `cargo test --workspace --features mock-hardware` CI command. No new file types, gates, or CI configuration.

## Platform Considerations

None identified. `Arc<NodeTypeRegistry>` is a Rust-level abstraction with no platform-specific I/O. The relocation to `anvilml-core` does not introduce any `#[cfg(unix)]`/`#[cfg(windows)]`-sensitive code. The Windows cross-check (`cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu`) exercises the same code paths as the Linux build.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Moving `NodeTypeRegistry` to `anvilml-core` is a deviation from P11-A1's original placement and from this task's own description (which assumes the registry stays in `anvilml-scheduler`). | High (confirmed necessary, not merely possible) | Medium | Document the cycle and the rejected alternative (removing the unused dependency) explicitly in this plan's Existing Codebase Assessment, rather than discovering it reactively mid-implementation. Amend `anvilml-core`'s crate doc honestly rather than silently violating its stated constraint. |
| `ManagedWorker::new()` already has 18 parameters; adding a 19th may read as excessive. | Low | Low | `#[allow(clippy::too_many_arguments)]` is already present on this test constructor for exactly this reason. The new parameter follows the same `Option<Arc<T>>` pattern as three existing fields. |
| `do_respawn`'s `node_registry` guard, if implemented as `.expect()` instead of mirroring the `routes` guard, would panic on any test worker built via `new()` with `node_registry: None` that also has `routes: Some(...)` and reaches the respawn path. | Medium | High | Mirror the existing `routes` guard exactly — return `Err(AnvilError::Internal(...))`, not `.expect()`. Verify against every existing test that exercises `do_respawn` (`test_respawn_cycle_entered_after_child_exit`) that its `routes` parameter is also `None`, so the `routes` guard fires first regardless — confirmed during implementation. |
| The existing "worker reached Ready" INFO log is missing 3 of 5 mandated fields, independent of this task. Leaving it unfixed would mean this task touches the exact code without satisfying `ENVIRONMENT.md §9`'s "every task that touches this subsystem must verify the log point" rule. | High (confirmed present) | Low | Fix it as part of this task — destructure the two additional fields (`torch_version`, `fp8`) already needed for the registry call, and add them plus `node_count` to the existing log call. |
| `TASKS_PHASE011.md`'s own "Known Constraints" section makes a claim about `is_empty()`'s behavior after an empty-vec update that may not match what P11-A1 actually implemented — this would only surface once the new test is written and run. | Medium | Medium | Verify `is_empty()`'s actual implementation against the claim before writing any assertion that depends on it, rather than trusting the task doc's prose. If a mismatch is found, resolve it at the implementation level (the registry needs the distinction P11-A3 depends on) rather than by weakening the test. |

## Acceptance Criteria

- [ ] `cargo test --workspace --features mock-hardware` exits 0 (all existing tests still pass, including the new one)
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` exits 0 (Windows cross-check)
- [ ] No `pub` item removed or renamed from `anvilml-worker`'s public surface (only existing signatures extended)