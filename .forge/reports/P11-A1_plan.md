# Plan Report: P11-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P11-A1                                            |
| Phase       | 11 — Dynamic Node System                          |
| Description | anvilml-worker: ManagedWorker calls node_registry.register_all() on Ready |
| Depends on  | P10-D1                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-07-06T12:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Connect the worker lifecycle to the dynamic node registry by adding an `Arc<NodeTypeRegistry>` parameter to `ManagedWorker::new()`, then calling `node_registry.register_all(event.node_types)` inside `handle_event()` when processing a `WorkerEvent::Ready`. This is the single call site that ever populates the registry, closing the gap Phase 10 left open by design.

## Scope

### In Scope
- Add `Arc<NodeTypeRegistry>` field to `ManagedWorker` struct.
- Add `Arc<NodeTypeRegistry>` field to `ManagedWorkerConfig` struct.
- Update `ManagedWorker::new()` to accept and store the new field from config.
- Update `handle_event()` to call `node_registry.register_all(event.node_types)` when processing `WorkerEvent::Ready`, before the existing status transition to `Idle`.
- Add ≥4 new integration tests in `crates/anvilml-worker/tests/managed_tests.rs`:
  1. Ready event populates the registry (verify `len() == N` after Ready with N descriptors).
  2. Ready event with empty node_types results in an empty registry (verify `is_empty()` is true).
  3. Second Ready call (simulating respawn) replaces rather than merges prior contents.
  4. Registry is correctly populated before status transitions to Idle (ordering invariant).

### Out of Scope
None. `defers_to (from JSON): []` — this task must implement its full scope. No deferrals permitted.

## Existing Codebase Assessment

**What already exists:** `NodeTypeRegistry` (Phase 3, `crates/anvilml-core/src/node_registry.rs`) is fully implemented with `register_all()` (replaces, does not merge), `get()`, `list()`, `len()`, `is_empty()`, and `Default`. It is `pub use`d from `anvilml-core::lib.rs`. `WorkerEvent::Ready` (Phase 3, `crates/anvilml-ipc/src/messages.rs`) already carries `node_types: Vec<NodeTypeDescriptor>`. `ManagedWorker::run_once()` already receives a `WorkerEvent::Ready` in its phase-1 loop and calls `handle_event()` for it; `handle_event()` currently sets status to `Idle` on Ready.

**Established patterns:** The codebase uses `Arc<T>` for shared ownership across the worker pool (e.g., `Arc<RouterTransport>`, `Arc<Demux>`, `Arc<dyn WorkerSpawner>`). `ManagedWorkerConfig` is a named-struct builder (12 fields, grouped by concern). Tests in `managed_tests.rs` use in-process ZeroMQ ROUTER/DEALER pairs with `MockWorkerSpawner` to simulate workers. The `test-utils` feature gate exposes `pub(crate)` items to integration tests.

**Gap between design doc and source:** The design doc (§10.2) says the registry is populated "only from worker Ready events" — this is the first task that actually implements that contract. Currently `register_all()` is never called from any production code path, so the registry is always empty.

## Resolved Dependencies

| Type   | Name           | Version verified | MCP source | Feature flags confirmed |
|--------|----------------|-----------------|------------|------------------------|
| crate  | NodeTypeRegistry (anvilml-core) | 0.1.26 (workspace internal) | Cargo.toml read | n/a (internal crate) |

No new external dependencies are introduced. `NodeTypeRegistry` is already re-exported from `anvilml-core` and `WorkerEvent::Ready` already carries `node_types`.

## Approach

### Step 1: Add `Arc<NodeTypeRegistry>` to `ManagedWorkerConfig`

In `crates/anvilml-worker/src/managed.rs`, add a new field to `ManagedWorkerConfig`:

```rust
/// Dynamic node type registry — populated from worker Ready events.
///
/// `register_all()` is called exactly once per Ready event (in `handle_event()`),
/// replacing the prior contents. This is the one and only call site that ever
/// populates the registry, per ANVILML_DESIGN.md §10.2.
pub node_registry: Arc<NodeTypeRegistry>,
```

The field goes in the "Shared infrastructure" group, after the existing `status` field, maintaining the grouping convention.

**Rationale:** `Arc<NodeTypeRegistry>` follows the established pattern used for `Arc<RouterTransport>` and `Arc<Demux>` — shared pool-wide infrastructure that is constructed once outside `ManagedWorker` and passed in via config.

### Step 2: Add `Arc<NodeTypeRegistry>` to `ManagedWorker` struct

Add a corresponding private field to `ManagedWorker`:

```rust
/// Dynamic node type registry — populated from worker Ready events.
///
/// `register_all()` is called in `handle_event()` on every Ready event.
node_registry: Arc<NodeTypeRegistry>,
```

Place it after the `demux` field (same concern: shared infrastructure).

### Step 3: Update `ManagedWorker::new()` to accept and store the field

In the `new()` method, add `self.node_registry = config.node_registry;` to the struct initialization. Also add the import at the top of the file:

```rust
use anvilml_core::NodeTypeRegistry;
```

### Step 4: Update `handle_event()` to call `register_all()` on Ready

In the `WorkerEvent::Ready { .. }` match arm of `handle_event()`, call `register_all()` **before** the existing status transition:

```rust
WorkerEvent::Ready { node_types, .. } => {
    // Populate the dynamic node registry from the worker's self-reported
    // node types. register_all() replaces (not merges) prior contents,
    // which is correct on respawn when the worker re-reports its full set.
    self.node_registry.register_all(node_types);
    *self.status.write().await = WorkerStatus::Idle;
    tracing::info!(worker_id = %self.worker_id, "worker_ready");
    false
}
```

**Rationale for ordering:** The registry must be populated *before* the status transitions to `Idle` — this ensures node types are available the moment the worker becomes usable, per the task's specification. The existing `tracing::info!` call stays as-is (it is the mandatory INFO log point for `worker_ready`).

**Logging note:** No new mandatory log points are introduced. The existing `worker_ready` INFO log at `handle_event()` already covers the Ready event lifecycle. The `register_all()` call is a data operation that does not require its own log call per the project's logging conventions (it is a deterministic, non-optional step of the Ready handler).

### Step 5: Write integration tests in `managed_tests.rs`

Add four new tests using the established test pattern (ROUTER/DEALER pair + `MockWorkerSpawner`):

**Test 1: `test_ready_event_populates_registry`**
- Construct a `NodeTypeRegistry`, wrap in `Arc`, pass via config.
- Spawn worker, send Ready event with 2 `NodeTypeDescriptor`s.
- After processing, assert `registry.len() == 2` and `registry.get("LoadModel")` returns the expected descriptor.

**Test 2: `test_ready_event_empty_node_types_cleans_registry`**
- Pre-populate the registry with one descriptor.
- Send Ready event with `node_types: vec![]`.
- Assert `registry.is_empty()` is true after processing.

**Test 3: `test_respawn_second_ready_replaces_not_merges`**
- Send first Ready with descriptors A and B.
- Cause worker to exit (send Dying).
- Re-spawn worker (via `run()` outer loop) and send second Ready with descriptor C only.
- Assert `registry.len() == 1` and `registry.get("A")` returns `None` (A was replaced, not merged).

**Test 4: `test_ready_populates_registry_before_idle_transition`**
- Send Ready event with descriptors.
- Use a short `tokio::time::sleep` then check both `registry.len() > 0` AND `status == Idle`.
- This verifies the ordering invariant: registry is populated before status transitions.

## Public API Surface

| Item | Crate/Module Path | Description |
|------|-------------------|-------------|
| `ManagedWorkerConfig::node_registry` | `anvilml-worker/src/managed.rs` | New `pub node_registry: Arc<NodeTypeRegistry>` field |
| `NodeTypeRegistry::register_all` (existing) | `anvilml-core/src/node_registry.rs` | Already pub; called from `handle_event()` — no signature change |

No new `pub` items are introduced. The only change to the public API surface is the addition of one field to an existing public config struct.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-worker/src/managed.rs` | Add `Arc<NodeTypeRegistry>` to config + struct; update `new()`; update `handle_event()` Ready arm |
| Modify | `crates/anvilml-worker/tests/managed_tests.rs` | Add ≥4 new integration tests |
| Bump | `crates/anvilml-worker/Cargo.toml` | Patch version 0.1.26 → 0.1.27 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-worker/tests/managed_tests.rs` | `test_ready_event_populates_registry` | Ready event with N descriptors populates registry: `len() == N`, `get()` returns correct descriptors | ROUTER/DEALER pair, MockWorkerSpawner, empty registry | Ready event with 2 NodeTypeDescriptors (`LoadModel`, `Sampler`) | `registry.len() == 2`, `registry.get("LoadModel")` matches | `cargo test -p anvilml-worker --test managed_tests test_ready_event_populates_registry` exits 0 |
| `crates/anvilml-worker/tests/managed_tests.rs` | `test_ready_event_empty_node_types_cleans_registry` | Ready with empty node_types results in empty registry | Pre-populated registry with 1 descriptor, ROUTER/DEALER pair | Ready event with `node_types: vec![]` | `registry.is_empty() == true` after processing | `cargo test -p anvilml-worker --test managed_tests test_ready_event_empty_node_types_cleans_registry` exits 0 |
| `crates/anvilml-worker/tests/managed_tests.rs` | `test_respawn_second_ready_replaces_not_merges` | Second Ready call replaces prior contents, does not merge | Worker runs through one generation, sends Dying, re-spawns via outer loop | First Ready with descriptors A+B, second Ready with descriptor C only | `registry.len() == 1`, `registry.get("A") == None`, `registry.get("C")` matches | `cargo test -p anvilml-worker --test managed_tests test_respawn_second_ready_replaces_not_merges` exits 0 |
| `crates/anvilml-worker/tests/managed_tests.rs` | `test_ready_populates_registry_before_idle_transition` | Registry is populated before status transitions to Idle | ROUTER/DEALER pair, MockWorkerSpawner | Ready event with 1 descriptor | After Ready: `registry.len() > 0` AND `status == Idle` both true | `cargo test -p anvilml-worker --test managed_tests test_ready_populates_registry_before_idle_transition` exits 0 |

## CI Impact

No CI changes required. The task only modifies source code and tests within the existing `anvilml-worker` crate. The `cargo test --workspace --features mock-hardware` CI job already picks up `managed_tests.rs` integration tests automatically.

## Platform Considerations

None identified. The `NodeTypeRegistry` is a pure data structure with no platform-specific code. The `Arc<NodeTypeRegistry>` field is a standard Rust shared-ownership type with identical behavior on Linux and Windows. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Adding `Arc<NodeTypeRegistry>` to `ManagedWorkerConfig` changes the struct layout — all existing callers of `ManagedWorker::new()` (outside of tests, e.g. production code in `backend/main.rs`) will fail to compile until updated. However, `backend/main.rs` is modified in a later phase task (P11-D1), so this compilation failure is expected and will be resolved by that task. | High | Medium | The compilation failure is intentional and expected — it is the correct "fail early" behavior. P11-D1 will construct the registry and pass it through. No mitigation needed beyond confirming P11-D1 exists as a prerequisite. |
| `handle_event()` borrows `self` mutably while `node_registry.register_all()` also needs `&self` — no conflict since `register_all()` takes `&self` (interior mutability via `RwLock`). However, if a future refactor changes `register_all` to take `&mut self`, this would break. | Low | Low | `register_all()` is confirmed via MCP to take `&self`. The `RwLock` provides interior mutability. This is a design invariant, not a runtime risk. |
| Test 3 (respawn replacement) requires the `run()` outer loop to execute a second generation within the test's bounded timeout. The mock spawner spawns `sleep 999` which is harmless, but the keepalive watchdog fires quickly in tests — need to ensure the second Ready event arrives before the watchdog times out. | Medium | Medium | Use short watchdog intervals (`Duration::from_millis(100)`) and send the second Ready event promptly after the worker re-spawns. The `run()` outer loop's backoff delay is also short in tests via `RespawnPolicy::new(100, 10, 50)`. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-worker --test managed_tests` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (no new warnings from the modified file)
- [ ] `cargo check --workspace --features mock-hardware` exits 0 (compilation succeeds on all cfg-gated paths)
