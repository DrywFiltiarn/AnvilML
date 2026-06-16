# Tasks: Phase 013 — Job Queue & Persistence

| Field | Value |
|-------|-------|
| Phase | 013 |
| Name | Job Queue & Persistence |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 12 |

## Overview

Phase 013 implements job queue & persistence as the next vertical slice. All tasks in this phase build on Phase 12 being complete. Each task implements one module or one concern, with tests, and leaves the binary in a runnable state.

Refer to `docs/ANVILML_DESIGN.md` for the full specification of types, interfaces, and contracts relevant to this phase.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Job Queue & Persistence | P13-A1…P13-A3 | Job Queue & Persistence implementation |
| B | Job Queue & Persistence | P13-B1 | queue.rs JobQueue FIFO with O(1) cancel |

## Prerequisites

Phase 12 complete. Refer to `docs/TASKS_PHASE012.md` for the terminal task and Runnable Proof of Phase 12.

## Task Descriptions

### P13-A1: anvilml-scheduler: queue.rs JobQueue FIFO with O(1) cancel

**Context:** Create crates/anvilml-scheduler/src/queue.rs: JobQueue{items:VecDeque<Job>,by_id:HashMap<Uuid,usize>}. pub fn push(&mut self,job:Job). pub fn pop_front(&mut self)->Option<Job>. pub fn cancel(&mut self,id:Uuid)->bool. pub fn get(&self,id:&Uuid)->Option<&Job>. pub fn list(&self)->Vec<&Job>. pub fn len(&self)->usize. cargo test -p anvilml-scheduler --features mock-hardware -- queue exits 0 >=6 tests.

**Acceptance criterion:** See context field — all stated commands must exit 0.

---

### P13-A2: anvilml-scheduler: ledger.rs VramLedger per-device reservation

**Context:** Create crates/anvilml-scheduler/src/ledger.rs: VramLedger{reservations:HashMap<u32,u32>,totals:HashMap<u32,u32>}. pub fn register_device(&mut self,index:u32,vram_total_mib:u32). pub fn would_fit(&self,index:u32,requested_mib:u32)->bool. pub fn reserve(&mut self,index:u32,mib:u32). pub fn release(&mut self,index:u32,mib:u32). tracing::debug!(device_index,reserved_mib,free_after_mib,"vram reserved")...

**Acceptance criterion:** See context field — all stated commands must exit 0.

---

### P13-A3: anvilml-scheduler: scheduler.rs JobScheduler submit and persistence

**Context:** Create crates/anvilml-scheduler/src/scheduler.rs: JobScheduler{queue:Arc<Mutex<JobQueue>>,ledger:Arc<Mutex<VramLedger>>,node_registry:Arc<NodeTypeRegistry>,db:SqlitePool,broadcaster:Arc<EventBroadcaster>}. pub async fn submit(&self,req:SubmitJobRequest)->Result<SubmitJobResponse>: validate_graph; create Job{status:Queued}; INSERT to SQLite; push to queue; broadcast WsEvent::JobQueued; notify dispa...

**Acceptance criterion:** See context field — all stated commands must exit 0.

---

### P13-B1: anvilml-server: POST /v1/jobs, GET /v1/jobs, GET /v1/jobs/:id wired to scheduler

**Context:** Update handlers/jobs.rs: submit_job now calls scheduler.submit(req).await returning real job_id+queue_position. Add list_jobs(Query<{status,limit,before}>) calling scheduler.list_jobs(). Add get_job(Path<Uuid>) returning 200 or 404. Add scheduler:Arc<JobScheduler> to AppState. Init scheduler in main.rs. Mount all three routes. POST valid graph -> 202 {job_id,queue_position:0}; GET /v1/jobs/:id -> ...

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
