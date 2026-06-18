# Plan Report: P11-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P11-A1                                        |
| Phase       | 011 — Dynamic Node Registry                   |
| Description | NodeTypeRegistry populated from WorkerEvent::Ready |
| Depends on  | none                                          |
| Project     | anvilml                                       |
| Planned at  | 2026-06-18T22:45:00Z                          |
| Attempt     | 1                                             |

## Objective

Create `NodeTypeRegistry` in `crates/anvilml-scheduler/src/node_registry.rs` — a thread-safe map from node type name to `NodeTypeDescriptor`, updated each time a worker sends its `Ready` event. The registry provides `update_from_worker`, `get`, `all_types`, and `is_empty` methods, with a `tracing::debug!` log call on each update. This is the foundation for graph validation in Phase 012 and the `GET /v1/nodes` endpoint in Phase 013.

## Scope

### In Scope
- Create `crates/anvilml-scheduler/src/node_registry.rs` with `NodeTypeRegistry` struct and all four methods.
- Update `crates/anvilml-scheduler/src/lib.rs` to declare `pub mod node_registry` and `pub use node_registry::NodeTypeRegistry`.
- Update `crates/anvilml-scheduler/Cargo.toml` to add `hashbrown = "1"` (for `HashMap`) dependency.
- Create `crates/anvilml-scheduler/tests/node_registry_tests.rs` with ≥ 4 tests.
- Bump `anvilml-scheduler` crate version from `0.1.0` to `0.1.1`.

### Out of Scope
- Wiring `NodeTypeRegistry` into `WorkerPool` / `ManagedWorker` (P11-A2).
- The `GET /v1/nodes` HTTP endpoint (P11-A3).
- DAG validation using the registry (Phase 012).
- Any Python-side changes.

## Existing Codebase Assessment

The `anvilml-scheduler` crate currently has only a stub `lib.rs` (11 lines) with a `pub fn stub()` placeholder. No source modules exist yet — `scheduler.rs`, `queue.rs`, `ledger.rs`, `dag.rs`, and `node_registry.rs` are all absent. The `tests/` directory does not exist either. This task establishes the first production module in the crate.

The `NodeTypeDescriptor` type already exists in `anvilml-core/src/types/node.rs` with fields `type_name`, `display_name`, `category`, `description`, `inputs`, and `outputs`. It derives `Debug, Clone, Default, Serialize, Deserialize, ToSchema`. The `WorkerEvent::Ready` variant (in `anvilml-ipc/src/messages.rs`) carries a `node_types: Vec<NodeTypeDescriptor>` field. The crate already depends on `tokio`, `tracing`, and `anvilml-core` — the only new dependency needed is `hashbrown` for `HashMap`.

The `TASKS_PHASE011.md` notes that `update_from_worker` should **merge**: existing types not present in the new list are preserved, because different workers may have different node sets. The task also mandates `tokio::sync::RwLock` (not `std::sync::RwLock`) because the methods are async.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | tokio   | 1.52.3          | Workspace      | full (already in Cargo.toml) |
| crate  | tracing | 0.1.44          | Workspace      | std, attributes (already in Cargo.toml) |
| crate  | anvilml-core | 0.1.0     | Workspace (path dep) | n/a |
| crate  | hashbrown | 1             | Workspace fallback (not yet declared) | n/a |

Note: `hashbrown` is not yet in the workspace dependencies. The plan uses `hashbrown = "1"` as a bare version string. The ACT agent should verify the latest stable 1.x version via MCP at session start and record it. If `hashbrown` is unavailable, `std::collections::HashMap` is an acceptable fallback since the registry is accessed under a lock guard and single-threaded access to `HashMap` is safe — but `hashbrown` is preferred for clarity of intent (the registry is a pure data structure).

## Approach

1. **Add `hashbrown` dependency to `Cargo.toml`.** Append `hashbrown = "1"` under `[dependencies]`. This is the only new dependency for this task. The workspace already declares `tokio`, `tracing`, and `anvilml-core`.

2. **Create `crates/anvilml-scheduler/src/node_registry.rs`.** Implement the following:

   a. **Struct definition:**
   ```rust
   use std::sync::Arc;
   use hashbrown::HashMap;
   use tokio::sync::RwLock;
   use anvilml_core::NodeTypeDescriptor;

   /// Thread-safe registry of node types, populated from worker `Ready` events.
   ///
   /// Stores a mapping from node type name (`String`) to `NodeTypeDescriptor`.
   /// Updated via `update_from_worker` when a worker reports its capabilities.
   /// Existing entries not present in the new list are preserved (merge semantics),
   /// because different workers may register different subsets of node types.
   #[derive(Debug, Default)]
   pub struct NodeTypeRegistry {
       types: Arc<RwLock<HashMap<String, NodeTypeDescriptor>>>,
   }
   ```

   b. **`impl NodeTypeRegistry` methods:**

   - `pub async fn new() -> Self` — returns `NodeTypeRegistry::default()` (empty map).
   - `pub async fn update_from_worker(&self, types: Vec<NodeTypeDescriptor>)` — acquires a write lock, inserts/updates each descriptor by `type_name`, then logs `tracing::debug!(node_count = types.len(), "node registry updated")`. The task description mentions `worker_id` in the log but this method does not receive a worker_id parameter. The ACT agent should add `worker_id: &str` as the first parameter: `pub async fn update_from_worker(&self, worker_id: &str, types: Vec<NodeTypeDescriptor>)` to match the log field requirement. This is a deviation from the minimal signature in the task description but required by the mandatory DEBUG log point in `ENVIRONMENT.md §9` ("Node registry: Registry updated from worker Ready — `worker_id=`, `node_count=`").
   - `pub async fn get(&self, type_name: &str) -> Option<NodeTypeDescriptor>` — acquires a read lock, looks up by key.
   - `pub async fn all_types(&self) -> Vec<NodeTypeDescriptor>` — acquires a read lock, collects all values into a vec.
   - `pub async fn is_empty(&self) -> bool` — acquires a read lock, checks `types.is_empty()`.

   c. **Doc comments** on the struct and all four `pub async fn` methods per `FORGE_AGENT_RULES.md §12.1`.

   d. **Inline comments** at decision points: the merge semantics in `update_from_worker` (explaining why existing entries are preserved — different workers may have different node sets).

3. **Update `crates/anvilml-scheduler/src/lib.rs`.** Replace the stub content with:
   ```rust
   //! Job scheduling, VRAM ledger, DAG validation, and dispatch for AnvilML.
   //!
   //! This crate owns the job queue (FIFO with O(1) cancel), VRAM ledger
   //! (per-device reservation tracking), DAG validation using the dynamic
   //! node type registry, and the dispatch loop.
   //!
   //! **Hard constraints:** No knowledge of HTTP request/response types.
   //! The scheduler speaks in jobs, graphs, and VRAM — not routes or handlers.

   pub mod node_registry;
   pub use node_registry::NodeTypeRegistry;
   ```
   This replaces the `#[allow(dead_code)] pub fn stub()` line. The crate-level doc comment is preserved verbatim.

4. **Create `crates/anvilml-scheduler/tests/node_registry_tests.rs`.** Write ≥ 4 tests:

   a. `test_update_populates_registry` — create a registry, call `update_from_worker` with 2 descriptors, verify `get` returns both by name, verify `all_types` returns 2 items, verify `is_empty` returns false.
   b. `test_get_returns_none_for_unknown_type` — create empty registry, call `get("unknown")`, verify `None`.
   c. `test_all_types_returns_all_descriptors` — update with 3 descriptors, call `all_types`, verify length is 3 and each descriptor matches.
   d. `test_is_empty_before_and_after_update` — verify `is_empty` is true on default registry, update with 1 descriptor, verify `is_empty` is false.
   e. `test_update_from_worker_merges` — update with type A, update with type B (no type A in second batch), verify both A and B are still present. This tests the merge semantics.

   Each test function gets a doc comment per `ENVIRONMENT.md §11.4`.

5. **Bump crate version.** Change `version.workspace = true` to `version = "0.1.1"` in `crates/anvilml-scheduler/Cargo.toml`. The workspace version remains `0.1.0` (read-only).

## Public API Surface

| Item | Path | Signature |
|------|------|-----------|
| struct | `crates/anvilml-scheduler/src/node_registry.rs:NodeTypeRegistry` | `pub struct NodeTypeRegistry { types: Arc<RwLock<HashMap<String, NodeTypeDescriptor>>> }` |
| fn | `node_registry.rs:NodeTypeRegistry::new` | `pub async fn new() -> Self` |
| fn | `node_registry.rs:NodeTypeRegistry::update_from_worker` | `pub async fn update_from_worker(&self, worker_id: &str, types: Vec<NodeTypeDescriptor>)` |
| fn | `node_registry.rs:NodeTypeRegistry::get` | `pub async fn get(&self, type_name: &str) -> Option<NodeTypeDescriptor>` |
| fn | `node_registry.rs:NodeTypeRegistry::all_types` | `pub async fn all_types(&self) -> Vec<NodeTypeDescriptor>` |
| fn | `node_registry.rs:NodeTypeRegistry::is_empty` | `pub async fn is_empty(&self) -> bool` |
| re-export | `lib.rs` | `pub use node_registry::NodeTypeRegistry` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-scheduler/src/node_registry.rs` | NodeTypeRegistry struct and all methods |
| MODIFY | `crates/anvilml-scheduler/src/lib.rs` | Replace stub; add `pub mod node_registry` and `pub use` |
| MODIFY | `crates/anvilml-scheduler/Cargo.toml` | Add `hashbrown = "1"` dependency; bump version to `0.1.1` |
| CREATE | `crates/anvilml-scheduler/tests/node_registry_tests.rs` | ≥ 4 unit tests for NodeTypeRegistry |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-scheduler/tests/node_registry_tests.rs` | `test_update_populates_registry` | `update_from_worker` inserts descriptors into the map; `get` returns them; `is_empty` becomes false | Default (empty) registry | `worker_id="test"`, two `NodeTypeDescriptor` values with distinct `type_name` | `get("LoadModel")` returns the descriptor; `is_empty() == false`; `all_types().len() == 2` | `cargo test -p anvilml-scheduler --features mock-hardware -- node_registry` exits 0 |
| `crates/anvilml-scheduler/tests/node_registry_tests.rs` | `test_get_returns_none_for_unknown_type` | `get` returns `None` for a type name that was never registered | Default (empty) registry | `type_name = "NonExistent"` | `get("NonExistent") == None` | `cargo test -p anvilml-scheduler --features mock-hardware -- node_registry` exits 0 |
| `crates/anvilml-scheduler/tests/node_registry_tests.rs` | `test_all_types_returns_all_descriptors` | `all_types` returns all registered descriptors with correct values | Registry populated with 3 descriptors | 3 `NodeTypeDescriptor` values with distinct `type_name` | `all_types().len() == 3`; each descriptor's `type_name` matches one of the inputs | `cargo test -p anvilml-scheduler --features mock-hardware -- node_registry` exits 0 |
| `crates/anvilml-scheduler/tests/node_registry_tests.rs` | `test_is_empty_before_and_after_update` | `is_empty` is `true` on default, `false` after `update_from_worker` | Default (empty) registry | `worker_id="test"`, one `NodeTypeDescriptor` | `is_empty() == true` before update; `is_empty() == false` after | `cargo test -p anvilml-scheduler --features mock-hardware -- node_registry` exits 0 |
| `crates/anvilml-scheduler/tests/node_registry_tests.rs` | `test_update_from_worker_merges` | `update_from_worker` preserves existing entries not in the new batch (merge semantics) | Registry with type A | First update: `[A]`; second update: `[B]` | After both updates, `get("A")` and `get("B")` both return `Some` | `cargo test -p anvilml-scheduler --features mock-hardware -- node_registry` exits 0 |

## CI Impact

No CI changes required. The new `tests/node_registry_tests.rs` file is automatically picked up by `cargo test --workspace --features mock-hardware` (the standard CI test command). No new file types, gates, or CI configuration are introduced.

## Platform Considerations

None identified. The `NodeTypeRegistry` uses only `tokio::sync::RwLock`, `hashbrown::HashMap`, and `serde`-serialisable domain types — all platform-neutral. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `hashbrown` not yet in workspace dependencies — the ACT agent must add it as a new dependency. If the MCP tool shows a different version than `"1"`, the plan must be adjusted. | Medium | Medium | The ACT agent verifies the version via MCP at session start. If `hashbrown` is unavailable, falls back to `std::collections::HashMap` (safe because all access is under an async lock guard, so concurrent access is serialized). |
| The task description specifies `update_from_worker(&self, types: Vec<NodeTypeDescriptor>)` without `worker_id`, but the mandatory DEBUG log point in ENVIRONMENT.md §9 requires `worker_id=`. Adding `worker_id: &str` as the first parameter changes the signature from what's specified. | Low | Low | Document this deviation in the plan. The ACT agent adds `worker_id: &str` to match the logging requirement. This is a minimal, additive change — no callers exist yet (P11-A2 wires it in the next task). |
| `Arc<RwLock<>>` pattern: the struct wraps the `RwLock` in `Arc`, and each method takes `&self` (not `&mut self`). The `RwLock` inside `Arc` provides interior mutability. If the ACT agent mistakenly uses `&mut self` receivers, the API will not be usable from shared references (e.g., from `WorkerPool` in P11-A2). | Low | High | Follow the exact signatures specified: all methods take `&self`. The `Arc<RwLock<>>` pattern ensures the struct is `Clone`-able and shareable across tasks. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --features mock-hardware -- node_registry` exits 0
- [ ] `cargo check --workspace --features mock-hardware` exits 0 (verifies no compile errors from new module)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (no clippy warnings)
- [ ] `cargo fmt --all -- --check` exits 0 (all files formatted)
- [ ] Report file begins with `# Plan Report: P11-A1` (verified by `head -1`)
- [ ] Report contains all 12 required section headings (verified by `grep "^## "`)
- [ ] Report is > 40 lines (verified by `wc -l`)
