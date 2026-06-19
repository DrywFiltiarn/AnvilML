# Tasks: Phase 011 — Dynamic Node Registry

| Field | Value |
|-------|-------|
| Phase | 011 |
| Name | Dynamic Node Registry |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 10 |

## Overview

Phase 011 introduces the dynamic node type registry. Before this phase, the set of available node types is empty and `GET /v1/nodes` does not exist. After this phase, each worker advertises its supported node types in the `Ready` IPC event, the scheduler accumulates those descriptors in a `NodeTypeRegistry`, and clients can query the full palette via `GET /v1/nodes`.

This registry is the prerequisite for graph validation in Phase 012: `validate_graph` checks every node type reference against the registry at submit time. No task written after Phase 011 may introduce a hardcoded node type name outside of test fixtures — the registry is the authoritative source from this point forward.

The Python side establishes the auto-import mechanism that makes node registration happen automatically when new `.py` files are added to `worker/nodes/`. The `@register` decorator and the `BaseNode` ABC are defined here; all node implementations in Phases 014 and 018 depend on them.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | anvilml-scheduler + server | P11-A1 … P11-A3 | NodeTypeRegistry, WorkerPool integration, GET /v1/nodes |
| B | Python worker | P11-B1 | NODE_REGISTRY auto-import, @register decorator, BaseNode ABC |

## Prerequisites

Phase 010 complete. `ManagedWorker` respawn logic is in place. The `Ready` IPC event exists in `anvilml-core` and is emitted by `worker_main.py` on startup. `WorkerPool` receives and dispatches `WorkerEvent::Ready`. `AppState` in `anvilml-server` can be extended with new fields.

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|-------------------|-------------------|-----------------|
| `ANVILML_DESIGN.md §5.7` | P11-A1 | `NodeTypeDescriptor` fields: `type_name`, `input_slots`, `output_slots`, `description` |
| `ANVILML_DESIGN.md §8.2` | P11-B1 | `WorkerEvent::Ready` `node_types` field type: `Vec<NodeTypeDescriptor>` |

## Task Descriptions

### Group A — anvilml-scheduler and anvilml-server

#### P11-A1: anvilml-scheduler: NodeTypeRegistry populated from WorkerEvent::Ready

**Goal:** Create `NodeTypeRegistry` in `crates/anvilml-scheduler/src/node_registry.rs` — a thread-safe map from node type name to `NodeTypeDescriptor` that is updated each time a worker sends its `Ready` event.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/node_registry.rs` — new file; `NodeTypeRegistry` struct and all methods
- `crates/anvilml-scheduler/src/lib.rs` — add `pub mod node_registry; pub use node_registry::NodeTypeRegistry`

**Key implementation notes:**
- `NodeTypeRegistry { types: Arc<RwLock<HashMap<String, NodeTypeDescriptor>>> }`
- Methods: `pub async fn update_from_worker(&self, types: Vec<NodeTypeDescriptor>)`, `pub async fn get(&self, type_name: &str) -> Option<NodeTypeDescriptor>`, `pub async fn all_types(&self) -> Vec<NodeTypeDescriptor>`, `pub async fn is_empty(&self) -> bool`
- `tracing::debug!(worker_id, node_count, "node registry updated")` in `update_from_worker`
- `update_from_worker` merges: existing types not present in the new list are preserved (workers may not all have the same node set)

**Acceptance criterion:** `cargo test -p anvilml-scheduler --features mock-hardware -- node_registry` exits 0 with ≥ 4 tests (update populates; get returns correct descriptor; all_types returns all; is_empty correct before/after update).

---

#### P11-A2: anvilml-worker: on Ready event update NodeTypeRegistry in scheduler

**Goal:** Wire `NodeTypeRegistry` into the `WorkerPool` / `ManagedWorker` event loop so that when a worker emits `WorkerEvent::Ready`, its `node_types` field is forwarded to the registry.

**Files to create or modify:**
- `crates/anvilml-worker/src/managed.rs` — extract `node_types` from `Ready` event; call `node_registry.update_from_worker()`
- `crates/anvilml-worker/src/pool.rs` — pass `Arc<NodeTypeRegistry>` into `spawn_all` and forward to each `ManagedWorker`

**Key implementation notes:**
- `WorkerPool::spawn_all` signature extends to accept `Arc<NodeTypeRegistry>`
- `tracing::info!(worker_id, node_count, "worker ready with node types")` logged on each Ready event per `ENVIRONMENT.md §9`
- In mock mode the `Ready` event carries an empty `node_types` vec; this is valid — the registry will be empty but `is_empty()` correctly reflects it

**Acceptance criterion:** `cargo test -p anvilml-worker --features mock-hardware` exits 0; test verifies that mock worker Ready event triggers `update_from_worker` and registry is non-nil (even if empty for mock).

---

#### P11-A3: anvilml-server: GET /v1/nodes listing registered node types

**Goal:** Expose `GET /v1/nodes` returning the current contents of `NodeTypeRegistry`. Returns 503 if the registry is empty (no worker has reached Ready yet). Returns 200 with an empty array after a mock worker reaches Ready (mock returns an empty `node_types` list, which is valid).

**Files to create or modify:**
- `crates/anvilml-server/src/handlers/nodes.rs` — new file; `list_nodes` handler
- `crates/anvilml-server/src/lib.rs` — mount `GET /v1/nodes`; add `node_registry: Arc<NodeTypeRegistry>` to `AppState`
- `crates/anvilml-server/src/state.rs` — add field to `AppState`

**Key implementation notes:**
- `list_nodes(State<AppState>) -> Result<Json<Vec<NodeTypeDescriptor>>, AnvilError>`
- If `!node_registry.has_been_updated().await`: return `AnvilError::WorkersUnavailable` (503)
- `is_empty()` is **not** the right check here — it only reflects the map's current contents, and a mock worker's empty-`node_types` `Ready` event leaves the map empty on purpose. `has_been_updated()` is the method that distinguishes "no worker has ever reached Ready" (503) from "a worker reached Ready and reported zero types" (200 `[]`) — see `anvilml_core::node_registry`'s module doc and `has_been_updated`'s own doc comment for the full rationale (this was corrected during P11-A2 after the distinction was found to not actually exist in P11-A1's original `is_empty()`-only implementation).
- After mock worker Ready: `has_been_updated()` is `true` (even though `is_empty()` is also still `true`), so response is 200 `[]`

**Acceptance criterion:** `cargo test -p anvilml-server --features mock-hardware` exits 0; test covers 503 before any worker Ready and 200 after mock worker Ready.

---

### Group B — Python worker

#### P11-B1: worker/nodes/__init__.py: NODE_REGISTRY auto-import and BaseNode ABC

**Goal:** Establish the node registration infrastructure on the Python side. `NODE_REGISTRY` is a module-level dict populated by `@register` decorators. All `.py` files in `worker/nodes/` are auto-imported on first access so decorator side-effects fire without manual import lists. `BaseNode` provides the ABC and `SlotSpec`/`NodeContext` are defined here for use by all node implementations.

**Files to create or modify:**
- `worker/nodes/__init__.py` — `NODE_REGISTRY` global; auto-import via `pkgutil.iter_modules`
- `worker/nodes/base.py` — `@register` decorator; `BaseNode` ABC with abstract `execute()`; `SlotSpec` dataclass; `NodeContext` dataclass
- `worker/worker_main.py` — update `_import_nodes()` to `import worker.nodes`; build `node_types` list from `NODE_REGISTRY`; include in `Ready` event

**Key implementation notes:**
- Auto-import must be idempotent and handle import errors gracefully (log warning, continue)
- `NodeContext` carries at minimum `job_id: str` and `emit: Callable` for progress events
- `SlotSpec(name: str, slot_type: str)` — slot type is a string matching values in `ANVILML_DESIGN.md §10.2`

**Acceptance criterion:** `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_nodes_base.py` exits 0 with ≥ 3 tests (registry populated after import; @register adds class; BaseNode cannot be instantiated directly).

---

## Phase Acceptance Criteria

```bash
cargo test --workspace --features mock-hardware
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
```

## Known Constraints and Gotchas

- `update_from_worker` is called from the worker event loop under an async context. The `RwLock` must be the `tokio::sync::RwLock`, not `std::sync::RwLock`.
- The mock `Ready` event sends `node_types: []` (empty vec). **`is_empty()` stays `true`** after `update_from_worker` is called with an empty vec — it only reflects the map's contents, and an empty-vec update inserts nothing. The "no worker has ever reached Ready" vs "a worker reached Ready with zero types" distinction that the 503 logic in P11-A3 needs is **not** something `is_empty()` can express; `NodeTypeRegistry` exposes a separate `has_been_updated()` method (added during P11-A2, backed by an internal flag set once on the first `update_from_worker` call) specifically for it. This line previously claimed `is_empty()` itself flips to `false` on an empty-vec update — it does not, and that claim was corrected after `test_managed_worker_forwards_to_node_registry` caught the contradiction against the real implementation.
- Follow `FORGE_AGENT_RULES.md §12` for all inline documentation.
- Follow `FORGE_AGENT_RULES.md §11` for all logging.
- Test isolation: every test that sets env vars must restore them unconditionally per `ENVIRONMENT.md §11.3`.
