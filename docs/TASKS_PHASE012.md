# Tasks: Phase 012 — Graph Validation

| Field | Value |
|-------|-------|
| Phase | 012 |
| Name | Graph Validation |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 11 |

## Overview

Phase 012 implements graph validation. Before this phase, `POST /v1/jobs` accepts any JSON payload and returns a placeholder job ID without inspecting the graph structure. After this phase, the submitted graph is validated against the dynamic node registry: every node type must be registered, edge references must resolve to real nodes and slots, slot types must be compatible, and the graph must be acyclic. All errors are collected before returning so the client sees the complete list of problems in one response.

The `GraphError` enum and `ValidatedGraph` newtype defined here are the types that Phase 013's `JobScheduler::submit` builds on. Getting the error representation right here means Phase 013 only needs to forward the result, not reparse it.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | anvilml-scheduler | P12-A1 … P12-A2 | validate_graph function, GraphError enum, ValidatedGraph newtype |
| B | anvilml-server | P12-B1 | POST /v1/jobs wired to validate_graph, 422 on invalid |

## Prerequisites

Phase 011 complete. `NodeTypeRegistry` exists and is populated from worker `Ready` events. `NodeTypeDescriptor` with `input_slots` and `output_slots` is defined in `anvilml-core`. `AnvilError::InvalidGraph(Vec<String>)` variant exists in `anvilml-core` (or is added in P12-A2). `AppState` carries `Arc<NodeTypeRegistry>`.

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|-------------------|-------------------|-----------------|
| `ANVILML_DESIGN.md §10.1` | P12-A1 | Graph JSON structure: `nodes` array, `edges` array, node `id`/`type`/`inputs` fields |
| `ANVILML_DESIGN.md §10.2` | P12-A1 | Slot type compatibility rules |
| `ANVILML_DESIGN.md §12.5` | P12-B1 | 422 error response shape: `{ error, message, request_id }` |

## Task Descriptions

### Group A — anvilml-scheduler

#### P12-A1: anvilml-scheduler: dag.rs validate_graph collect-all-errors mode

**Goal:** Implement `validate_graph` in `crates/anvilml-scheduler/src/dag.rs`. The function is non-fail-fast: it collects all errors before returning so callers receive the complete problem list in one call.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/dag.rs` — new file; `ValidatedGraph` newtype and `validate_graph` function
- `crates/anvilml-scheduler/src/lib.rs` — add `pub mod dag`
- `crates/anvilml-scheduler/tests/dag_tests.rs` — new file; ≥ 8 tests

**Key implementation notes:**
- `pub struct ValidatedGraph(pub serde_json::Value)` — wraps the original graph JSON; construction is private to this module
- `pub async fn validate_graph(graph: &Value, registry: &NodeTypeRegistry) -> Result<ValidatedGraph, Vec<String>>`
- Checks in order (all collected, non-fail-fast): (1) `nodes` array present; (2) no duplicate node `id`; (3) every node `type` in `NodeTypeRegistry`; (4) every edge `{node_id, output_slot}` references an existing node and slot; (5) slot type compatibility per `ANVILML_DESIGN.md §10.2`; (6) acyclic via Kahn's algorithm
- Error strings are human-readable and name the specific offending node/slot/type

**Acceptance criterion:** `cargo test -p anvilml-scheduler --features mock-hardware -- dag` exits 0 with ≥ 8 tests (missing nodes array; duplicate ids; unknown type; bad edge ref; slot type mismatch; cycle; valid graph returns Ok; multiple errors collected at once).

---

#### P12-A2: anvilml-scheduler: GraphError enum and types.rs

**Goal:** Define `GraphError` as a typed enum for all graph validation failure modes. Re-export `ValidatedGraph` from `types.rs` to give the rest of the crate a single import point. Update `dag.rs` to use `GraphError` internally and convert to strings for the public return type.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/types.rs` — new file; `GraphError` enum; re-export `ValidatedGraph`
- `crates/anvilml-scheduler/src/dag.rs` — update to use `GraphError` internally
- `crates/anvilml-scheduler/src/lib.rs` — add `pub mod types; pub use types::GraphError`

**Key implementation notes:**
- `GraphError` variants: `UnknownNodeType(String)`, `DuplicateNodeId(String)`, `UnknownEdgeRef { node_id: String, slot: String }`, `SlotTypeMismatch { from: SlotType, to: SlotType }`, `CycleDetected(Vec<String>)`
- `ValidatedGraph` is defined in `dag.rs` and re-exported from `types.rs`; the definition does not move
- `impl Display for GraphError` provides the human-readable string used in the `Vec<String>` return type of `validate_graph`

**Acceptance criterion:** `cargo test -p anvilml-scheduler --features mock-hardware` exits 0; existing `dag_tests.rs` tests continue to pass without modification.

---

### Group B — anvilml-server

#### P12-B1: anvilml-server: POST /v1/jobs validating graph, 422 on invalid

**Goal:** Replace the placeholder `submit_job` handler with one that calls `validate_graph` and returns 422 on failure. On success, return 202 with a placeholder `job_id` (full persistence comes in Phase 013).

**Files to create or modify:**
- `crates/anvilml-server/src/handlers/jobs.rs` — create or replace; `submit_job` handler with validation
- `crates/anvilml-server/src/lib.rs` — mount `POST /v1/jobs` in `build_router`

**Key implementation notes:**
- Handler signature: `submit_job(State<AppState>, Json<SubmitJobRequest>) -> Result<(StatusCode, Json<SubmitJobResponse>), AnvilError>`
- If `node_registry.is_empty().await`: return `AnvilError::WorkersUnavailable` (503) before attempting validation
- Call `validate_graph(&req.graph, &state.node_registry).await`; on `Err(errors)`: return `AnvilError::InvalidGraph(errors)` → 422
- On `Ok`: return `202 { job_id: Uuid::new_v4(), queue_position: 0 }` (placeholder; real persistence in Phase 013)
- Integration tests: POST graph with unknown node type → 422; POST valid graph → 202

**Acceptance criterion:** `cargo test -p anvilml-server --features mock-hardware` exits 0; tests verify 503 when workers unavailable, 422 with error list on invalid graph, and 202 on valid graph.

---

## Phase Acceptance Criteria

```bash
cargo test --workspace --features mock-hardware
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
```

## Known Constraints and Gotchas

- `validate_graph` receives `&NodeTypeRegistry` not `Arc<NodeTypeRegistry>` — callers dereference before passing. This keeps `dag.rs` free of `Arc` machinery.
- The acyclicity check (Kahn's algorithm) must name the nodes involved in the cycle in the error string, not just report "cycle detected". This is required for the error message to be actionable.
- Follow `FORGE_AGENT_RULES.md §12` for all inline documentation: every `pub` item needs a doc comment; every decision point needs an inline comment.
- Follow `FORGE_AGENT_RULES.md §11` for all logging.
- Test isolation: every test that sets env vars must restore them unconditionally per `ENVIRONMENT.md §11.3`.
