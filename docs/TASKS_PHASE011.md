# Tasks: Phase 011 — Dynamic Node Registry

| Field | Value |
|-------|-------|
| Phase | 011 |
| Name | Dynamic Node Registry |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 10 |

## Overview

Phase 011 implements dynamic node registry as the next vertical slice. All tasks in this phase build on Phase 10 being complete. Each task implements one module or one concern, with tests, and leaves the binary in a runnable state.

Refer to `docs/ANVILML_DESIGN.md` for the full specification of types, interfaces, and contracts relevant to this phase.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Dynamic Node Registry | P11-A1…P11-A3 | Dynamic Node Registry implementation |
| B | Dynamic Node Registry | P11-B1 | NodeTypeRegistry populated from WorkerEvent |

## Prerequisites

Phase 10 complete. Refer to `docs/TASKS_PHASE010.md` for the terminal task and Runnable Proof of Phase 10.

## Task Descriptions

### P11-A1: anvilml-scheduler: NodeTypeRegistry populated from WorkerEvent::Ready

**Context:** Create crates/anvilml-scheduler/src/node_registry.rs: NodeTypeRegistry{types:Arc<RwLock<HashMap<String,NodeTypeDescriptor>>>}. pub async fn update_from_worker(&self,types:Vec<NodeTypeDescriptor>). pub async fn get(&self,type_name:&str)->Option<NodeTypeDescriptor>. pub async fn all_types(&self)->Vec<NodeTypeDescriptor>. pub async fn is_empty(&self)->bool. tracing::debug!(worker_id,node_count,"node ...

**Acceptance criterion:** See context field — all stated commands must exit 0.

---

### P11-A2: anvilml-worker: on Ready event update NodeTypeRegistry in scheduler

**Context:** Extend WorkerPool/ManagedWorker: when Ready event received extract node_types field; call node_registry.update_from_worker(node_types). Pass Arc<NodeTypeRegistry> into WorkerPool::spawn_all. tracing::info!(worker_id,node_count) logged on Ready per ENVIRONMENT.md §9. cargo test -p anvilml-worker --features mock-hardware exits 0.

**Acceptance criterion:** See context field — all stated commands must exit 0.

---

### P11-A3: anvilml-server: GET /v1/nodes listing registered node types

**Context:** Create handlers/nodes.rs: list_nodes(State<AppState>)->Json<Vec<NodeTypeDescriptor>> calling node_registry.all_types().await. If registry.is_empty: return 503 workers_unavailable (registry not yet populated). Mount GET /v1/nodes in build_router. Add node_registry:Arc<NodeTypeRegistry> to AppState. curl /v1/nodes -> 200 JSON array after worker Ready (mock returns empty node_types; 200 with []). car...

**Acceptance criterion:** See context field — all stated commands must exit 0.

---

### P11-B1: worker/nodes/__init__.py: NODE_REGISTRY auto-import

**Context:** Create worker/nodes/__init__.py: NODE_REGISTRY:dict[str,type]={} global. Auto-import all .py files in nodes/ directory via pkgutil.iter_modules to trigger @register decorators. Create worker/nodes/base.py: @register decorator adds class to NODE_REGISTRY; BaseNode ABC with execute() abstract; SlotSpec dataclass; NodeContext dataclass. Update worker_main.py _import_nodes(): import worker.nodes; buil...

**Acceptance criterion:** See context field — all stated commands must exit 0.

---

## Phase Acceptance Criteria

```bash
cargo test --workspace --features mock-hardware
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
```

## Known Constraints and Gotchas

- Follow `FORGE_AGENT_RULES.md §12` for all inline documentation: every pub item needs a doc comment; every decision point needs an inline comment.
- Follow `FORGE_AGENT_RULES.md §11` for all logging: mandatory INFO and DEBUG log points must be present before a task is marked complete.
- Test isolation: every test that sets env vars must restore them unconditionally per `ENVIRONMENT.md §11.3`.
