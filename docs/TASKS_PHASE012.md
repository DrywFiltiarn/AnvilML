# Tasks: Phase 012 — Graph Validation

| Field | Value |
|-------|-------|
| Phase | 012 |
| Name | Graph Validation |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 11 |

## Overview

Phase 012 implements graph validation as the next vertical slice. All tasks in this phase build on Phase 11 being complete. Each task implements one module or one concern, with tests, and leaves the binary in a runnable state.

Refer to `docs/ANVILML_DESIGN.md` for the full specification of types, interfaces, and contracts relevant to this phase.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Graph Validation | P12-A1…P12-A2 | Graph Validation implementation |
| B | Graph Validation | P12-B1 | dag.rs validate_graph collect-all-errors mode |

## Prerequisites

Phase 11 complete. Refer to `docs/TASKS_PHASE011.md` for the terminal task and Runnable Proof of Phase 11.

## Task Descriptions

### P12-A1: anvilml-scheduler: dag.rs validate_graph collect-all-errors mode

**Context:** Create crates/anvilml-scheduler/src/dag.rs: pub struct ValidatedGraph(pub Value). pub async fn validate_graph(graph:&Value,registry:&NodeTypeRegistry)->Result<ValidatedGraph,Vec<String>>. Checks (collect all, non-fail-fast): 1) nodes array present; 2) no duplicate id; 3) type in NodeTypeRegistry; 4) edge refs {node_id,output_slot} reference existing nodes+slots; 5) slot_type compatibility; 6) acyc...

**Acceptance criterion:** See context field — all stated commands must exit 0.

---

### P12-A2: anvilml-scheduler: GraphError enum and types.rs ValidatedGraph

**Context:** Create crates/anvilml-scheduler/src/types.rs: GraphError enum{UnknownNodeType(String),DuplicateNodeId(String),UnknownEdgeRef{node_id:String,slot:String},SlotTypeMismatch{from:SlotType,to:SlotType},CycleDetected(Vec<String>)}. ValidatedGraph newtype already in dag.rs - re-export from types.rs. Update dag.rs to use GraphError. cargo test -p anvilml-scheduler --features mock-hardware exits 0.

**Acceptance criterion:** See context field — all stated commands must exit 0.

---

### P12-B1: anvilml-server: POST /v1/jobs validating graph, 422 on invalid

**Context:** Create handlers/jobs.rs: submit_job(State<AppState>,Json<SubmitJobRequest>)->Result<(StatusCode,Json<SubmitJobResponse>),AnvilError>. If node_registry.is_empty: return 503 workers_unavailable. Call validate_graph(&req.graph,&node_registry).await. On Err(errors): return 422 AnvilError::InvalidGraph(errors). On Ok: return 202 with placeholder job_id. Mount POST /v1/jobs in build_router. POST unknown...

**Acceptance criterion:** See context field — all stated commands must exit 0.

---

## Phase Acceptance Criteria

```bash
cargo test --workspace --features mock-hardware
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
```

## Known Constraints and Gotchas

- Follow `FORGE_AGENT_RULES.md §12` for all inline documentation: every pub item needs a doc comment; every decision point needs an inline comment.
- Follow `FORGE_AGENT_RULES.md §11` for all logging: mandatory INFO and DEBUG log points must be present before a task is marked complete.
- Test isolation: every test that sets env vars must restore them unconditionally per `ENVIRONMENT.md §11.3`.
