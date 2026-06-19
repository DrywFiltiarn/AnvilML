# Implementation Report: P11-A2

| Field         | Value                                                              |
|---------------|---------------------------------------------------------------------|
| Task ID       | P11-A2                                                              |
| Phase         | 011 — Dynamic Node Registry                                         |
| Description   | anvilml-worker: on Ready event update NodeTypeRegistry in scheduler |
| Implemented   | 2026-06-19T12:40:00Z                                                |
| Status        | COMPLETE                                                            |

## Summary

Wired `NodeTypeRegistry` into the `ManagedWorker` / `WorkerPool` event loop so that when a worker emits `WorkerEvent::Ready`, its `node_types` field is forwarded to `NodeTypeRegistry::update_from_worker()`. This closes the loop between P11-A1 (registry creation) and the worker event system. `NodeTypeRegistry` was moved from `anvilml-scheduler` to `anvilml-core` to break a cyclic dependency (`anvilml-scheduler` depends on `anvilml-worker`, and `anvilml-worker` needed to call `update_from_worker`) — `anvilml-scheduler` now re-exports the type from `anvilml-core`. The existing "worker reached Ready" INFO log, found during implementation to be missing 3 of 5 `ENVIRONMENT.md §9`-mandated fields, was corrected in the same pass. During test-writing, a contradiction was found between `docs/TASKS_PHASE011.md`'s "Known Constraints" section and `is_empty()`'s actual P11-A1 implementation; rather than write a test against the wrong claim, `NodeTypeRegistry` gained a new `has_been_updated()` method that genuinely provides the never-updated-vs-updated-with-nothing distinction P11-A3 needs (see Deviations). All workspace tests pass, all gates pass.

## Resolved Dependencies

| Type   | Name              | Version resolved | Source                    |
|--------|-------------------|--------------------|----------------------------|
| crate  | anvilml-core      | 0.1.14              | Cargo.lock (workspace path dep, version bumped this task) |
| crate  | anvilml-scheduler | 0.1.2                | Cargo.lock (workspace path dep, version bumped this task) |
| crate  | hashbrown         | 0.17                 | Cargo.lock (relocated from anvilml-scheduler to anvilml-core, version unchanged) |
| crate  | tokio             | 1.52.3                | Cargo.lock (already workspace-resolved; newly declared as a direct dependency of anvilml-core) |

No new external crates were introduced. `hashbrown` and `tokio` moved from being declared dependencies of `anvilml-scheduler` to being declared dependencies of `anvilml-core`, at the versions already resolved in `Cargo.lock` from P11-A1 and earlier phases respectively — no version bump for either crate itself.

## Files Changed

| Action | Path | Description |
|--------|------|--------------|
| CREATE | `crates/anvilml-core/src/node_registry.rs` | `NodeTypeRegistry`, moved from `anvilml-scheduler`; gained `updated: AtomicBool` field and `has_been_updated()` method (see Deviations) |
| DELETE | `crates/anvilml-scheduler/src/node_registry.rs` | Moved to `anvilml-core` |
| MODIFY | `crates/anvilml-core/src/lib.rs` | Added `pub mod node_registry` + re-export; amended crate doc's "zero async" claim to state the one narrow, named exception |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Added `hashbrown = "0.17"`, `tokio = { workspace = true }`; bumped version `0.1.13` → `0.1.14` |
| MODIFY | `crates/anvilml-scheduler/src/lib.rs` | `pub mod node_registry; pub use node_registry::NodeTypeRegistry;` replaced with `pub use anvilml_core::NodeTypeRegistry;` |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Removed `hashbrown`; bumped version `0.1.1` → `0.1.2` |
| MODIFY | `crates/anvilml-scheduler/tests/node_registry_tests.rs` | Added `test_has_been_updated_distinguishes_never_updated_from_empty_update` (see Deviations) |
| MODIFY | `crates/anvilml-worker/src/managed.rs` | Added `node_registry` field; extended `new()` (18→19 params), `spawn()` (6→7 params), `do_respawn` (registry guard + forward); wired `update_from_worker` call into `run()`'s `Ready` arm; fixed the existing INFO log's missing fields |
| MODIFY | `crates/anvilml-worker/src/pool.rs` | Extended `spawn_all()` (4→5 params); forwards `Arc::clone(&node_registry)` into each `ManagedWorker::spawn()` call |
| MODIFY | `crates/anvilml-worker/Cargo.toml` | Bumped version `0.1.22` → `0.1.23` |
| MODIFY | `backend/src/main.rs` | Constructs `Arc::new(NodeTypeRegistry::new().await)`; passes into `spawn_all()` |
| MODIFY | `backend/Cargo.toml` | Bumped version `0.1.13` → `0.1.14` |
| MODIFY | `crates/anvilml-worker/tests/managed_tests.rs` | Added `node_registry` (19th) argument to all 7 `ManagedWorker::new()` call sites |
| MODIFY | `crates/anvilml-worker/tests/pool_tests.rs` | Added `node_registry` argument to `make_test_worker`; added `test_managed_worker_forwards_to_node_registry` |
| MODIFY | `crates/anvilml-server/tests/workers_tests.rs` | Added `node_registry` argument to its one `ManagedWorker::new()` call site |
| MODIFY | `docs/TASKS_PHASE011.md` | Corrected P11-A3's "Key implementation notes" (was pointing at `is_empty()` for 503-vs-200; now points at `has_been_updated()`) and the self-contradicting "Known Constraints" entry (see Deviations) |
| MODIFY | `docs/TESTS.md` | Added entries for both new tests |

## Commit Log

```
 .forge/reports/P11-A2_plan.md                            | 155 +++++++++++++++++++
 backend/Cargo.toml                                         |   2 +-
 backend/src/main.rs                                        |   9 ++
 crates/anvilml-core/Cargo.toml                              |   5 +-
 crates/anvilml-core/src/lib.rs                              |  19 ++-
 crates/anvilml-core/src/node_registry.rs                    | 146 ++++++++++++++++++
 crates/anvilml-scheduler/Cargo.toml                         |   3 +-
 crates/anvilml-scheduler/src/lib.rs                         |  11 +-
 crates/anvilml-scheduler/src/node_registry.rs               |  78 -----------
 crates/anvilml-scheduler/tests/node_registry_tests.rs       |  48 +++++++
 crates/anvilml-server/tests/workers_tests.rs                |   1 +
 crates/anvilml-worker/Cargo.toml                            |   2 +-
 crates/anvilml-worker/src/managed.rs                        |  82 +++++++++-
 crates/anvilml-worker/src/pool.rs                           |  14 +-
 crates/anvilml-worker/tests/managed_tests.rs                |  10 ++
 crates/anvilml-worker/tests/pool_tests.rs                   | 171 +++++++++++++++++-
 docs/TASKS_PHASE011.md                                       |   7 +-
 docs/TESTS.md                                                |  18 ++
 18 files changed, 683 insertions(+), 98 deletions(-)
```

## Test Results

```
     Running tests/pool_tests.rs (target/debug/deps/pool_tests-e266f2a527153482)

running 6 tests
test test_reexport_worker_pool ... ok
test test_broadcaster_returns_reference ... ok
test test_shutdown_all_completes_against_inert_handles ... ok
test test_managed_worker_forwards_to_node_registry ... ok
test test_spawn_all_workers_idle ... ok
test test_pool_broadcasts_status_change ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

```
     Running tests/node_registry_tests.rs (target/debug/deps/node_registry_tests-...)

running 6 tests
test test_get_returns_none_for_unknown_type ... ok
test test_is_empty_before_and_after_update ... ok
test test_all_types_returns_all_descriptors ... ok
test test_update_from_worker_merges ... ok
test test_update_populates_registry ... ok
test test_has_been_updated_distinguishes_never_updated_from_empty_update ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

```
     Running tests/managed_tests.rs (target/debug/deps/managed_tests-...)

running 12 tests
test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace: 194 tests passed, 0 failed (192 before this task; net +2 new tests — `test_managed_worker_forwards_to_node_registry` and `test_has_been_updated_distinguishes_never_updated_from_empty_update`).

## Format Gate

```
cargo fmt --all -- --check
```
Exit 0 — no formatting drift.

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
# → Finished `dev` profile [unoptimized + debuginfo] target(s)

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
# → Finished `dev` profile [unoptimized + debuginfo] target(s)
```

Both cross-checks exit 0. `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0.

## Project Gates

**Gate 1 — Config Surface Sync:** Not triggered. This task does not modify `ServerConfig` or any config field.

**Gate 2 — OpenAPI Drift:** Not triggered. This task does not modify handler function signatures, `#[utoipa::path]` annotations, `ToSchema` derives, or `AppState` fields used in response types — `AppState` itself is untouched (P11-A3's scope, not this task's).

**Gate 3 — Node Parity:** Not triggered. This task wires existing `NodeTypeDescriptor` values through to a registry; it does not add, remove, or rename node types in `worker/nodes/`.

## Public API Delta

```
# anvilml-core
+pub mod node_registry;
+pub use node_registry::NodeTypeRegistry;
+pub struct NodeTypeRegistry {
+    pub async fn new() -> Self {
+    pub async fn update_from_worker(&self, worker_id: &str, types: Vec<NodeTypeDescriptor>) {
+    pub async fn get(&self, type_name: &str) -> Option<NodeTypeDescriptor> {
+    pub async fn all_types(&self) -> Vec<NodeTypeDescriptor> {
+    pub async fn is_empty(&self) -> bool {
+    pub async fn has_been_updated(&self) -> bool {     # new in this task — see Deviations

# anvilml-scheduler
-pub mod node_registry;
-pub use node_registry::NodeTypeRegistry;
+pub use anvilml_core::NodeTypeRegistry;                 # re-export, same import path for existing callers

# anvilml-worker (existing pub items, signatures extended — not new items)
 ManagedWorker::new(..., ready_tx: Option<oneshot::Sender<()>>)
+ManagedWorker::new(..., ready_tx: Option<oneshot::Sender<()>>, node_registry: Option<Arc<NodeTypeRegistry>>)
 ManagedWorker::spawn(..., restart_rx: tokio::sync::watch::Receiver<u64>)
+ManagedWorker::spawn(..., restart_rx: tokio::sync::watch::Receiver<u64>, node_registry: Arc<NodeTypeRegistry>)
 WorkerPool::spawn_all(cfg, devices, transport, broadcaster)
+WorkerPool::spawn_all(cfg, devices, transport, broadcaster, node_registry: Arc<NodeTypeRegistry>)
```

`WorkerPool::new()` (the test constructor) is unchanged — confirmed during planning that it never constructs a `ManagedWorker` and so has no `node_registry` to thread through.

One new public item beyond the plan's Public API Surface table: `NodeTypeRegistry::has_been_updated()`. See Deviations for why.

## Deviations from Plan

1. **`NodeTypeRegistry` moved from `anvilml-scheduler` to `anvilml-core`.** Per the plan's own Existing Codebase Assessment — this was identified during planning (not discovered reactively mid-implementation) as the only resolution that doesn't defer the dependency cycle to a future task. `anvilml-core`'s crate doc was amended to state this exception explicitly rather than silently violate its "zero async" claim.

2. **`NodeTypeRegistry::has_been_updated()` added — not in the original P11-A1 API or this task's plan.** While writing `test_managed_worker_forwards_to_node_registry`, an assertion based on `docs/TASKS_PHASE011.md`'s "Known Constraints" section (which claims `is_empty()` "returns false after `update_from_worker` is called with an empty vec") failed: `is_empty()`'s actual P11-A1 implementation is `self.types.read().await.is_empty()` — purely a reflection of the map's contents, with no mechanism to distinguish "never called" from "called with an empty `Vec`". The task doc's claim was never true of the code as P11-A1 wrote it, and contradicted that same doc's own P11-A2 section two paragraphs above it ("the registry will be empty but `is_empty()` correctly reflects it" — i.e., stays `true`).

   Since P11-A3's planned `GET /v1/nodes` 503-vs-200 logic genuinely needs this distinction (503 only when no worker has ever reached `Ready`; 200 `[]` when a worker reached `Ready` with zero node types, as the mock worker does), the fix was made at the implementation level rather than by writing a test against incorrect behavior or silently dropping the assertion: `NodeTypeRegistry` gained an `updated: AtomicBool` field, set once (and never unset) on the first `update_from_worker` call, exposed via a new `has_been_updated()` method. `is_empty()` itself is unchanged — its doc comment now states explicitly what it does not distinguish, pointing callers needing that distinction at the new method.

   `docs/TASKS_PHASE011.md` was corrected in two places: P11-A3's "Key implementation notes" (was instructing the future implementer to gate 503 on `is_empty()`, which would have made every mock-mode response 503 forever) now points at `has_been_updated()`; the "Known Constraints" entry that originated the wrong claim now states the real, verified contract.

   A focused unit test (`test_has_been_updated_distinguishes_never_updated_from_empty_update`) was added to `node_registry_tests.rs` alongside P11-A1's existing registry tests, and the test that surfaced the issue (`test_managed_worker_forwards_to_node_registry`) asserts both `is_empty()`'s and `has_been_updated()`'s correct, distinct behavior across an empty-vec update.

3. **Existing "worker reached Ready" INFO log fixed.** Found during implementation (not flagged in the plan as a confirmed pre-existing gap, only as a risk) to be missing `torch_version` and `node_count` — only `worker_id` and `device` were present against `ENVIRONMENT.md §9`'s five-field requirement. Fixed in the same match arm already being edited to add the registry call, since both changes need the same two additional destructured fields (`torch_version`, `node_types`/`fp8`).

4. **`update_from_worker` called unconditionally on every `Ready` event, not only on the `Initializing → Idle` transition.** The plan's Approach section specified this; confirmed correct during implementation and called out here because it is easy to misread the surrounding `match *s { WorkerStatus::Initializing => ..., _ => ... }` as the natural place to gate the registry call too. Node types are a property of the `Ready` event itself, independent of which status transition (if any) accompanies it.

5. **Test approach for the new wiring test.** Per the plan's Approach step 11 fallback clause: the natural design (drive a real `Ready` event through `run()`'s `select!` loop) was attempted and abandoned after a documented, unresolved event-delivery failure mode consistent with — though not conclusively proven to be — the `spawn_run`/`_shutdown_tx` footgun already described in `managed_tests.rs`'s doc comments. `test_managed_worker_forwards_to_node_registry` instead constructs a `ManagedWorker` with `Some(registry)` (proving the field/parameter wiring compiles and is positioned correctly) and calls `update_from_worker` directly with the same arguments `run()`'s `Ready` arm would pass. The test's own doc comment states this limitation and what it does and does not prove, and points to `test_run_ready_event_releases_keepalive_gate` (`managed_tests.rs`) as existing coverage for a real `Ready` event successfully reaching this exact match arm via `run()`'s loop, for the adjacent `ready_tx` side effect.

## Blockers

None.