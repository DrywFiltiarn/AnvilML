# Tasks: Phase 010 — Worker Crash Recovery

| Field | Value |
|-------|-------|
| Phase | 010 |
| Name | Worker Crash Recovery |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 9 |

## Overview

Phase 010 implements worker crash recovery as the next vertical slice. All tasks in this phase build on Phase 9 being complete. Each task implements one module or one concern, with tests, and leaves the binary in a runnable state.

Refer to `docs/ANVILML_DESIGN.md` for the full specification of types, interfaces, and contracts relevant to this phase.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Worker Crash Recovery | P10-A1…P10-A2 | Worker Crash Recovery implementation |
| B | Worker Crash Recovery | P10-B1 | respawn.rs RespawnPolicy with backoff and max-attempt guard |

## Prerequisites

Phase 9 complete. Refer to `docs/TASKS_PHASE009.md` for the terminal task and Runnable Proof of Phase 9.

## Task Descriptions

### P10-A1: anvilml-worker: respawn.rs RespawnPolicy with backoff and max-attempt guard

**Context:** Create crates/anvilml-worker/src/respawn.rs: RespawnPolicy{delay_ms:u64,max_attempts:u32,window_s:u32}. pub fn should_respawn(&self,crash_count:u32,last_crash:Instant)->bool. pub fn next_delay_ms(&self,attempt:u32)->u64 (exponential backoff capped at 30s). Tests: max_attempts exceeded->false; within window->true; delay sequence correct. cargo test -p anvilml-worker --features mock-hardware -- resp...

**Acceptance criterion:** See context field — all stated commands must exit 0.

---

### P10-A2: anvilml-worker: managed.rs crash detection and automatic respawn

**Context:** Extend managed.rs: child.wait() future in managed run loop detects unexpected exit; transition Dead; broadcast WorkerStatusChanged(Dead); if in-flight job: mark Failed with error=worker_crashed via job store callback. After respawn_delay: if RespawnPolicy::should_respawn: transition Respawning; re-run spawn(); emit WorkerStatusChanged(Respawning). tracing::info!(worker_id,exit_code) on Dead; traci...

**Acceptance criterion:** See context field — all stated commands must exit 0.

---

### P10-B1: anvilml-server: POST /v1/workers/:id/restart handler

**Context:** Create handlers/workers.rs restart_worker(State<AppState>,Path<String>): lookup worker by id in WorkerPool; if not found 404; send Shutdown IPC message; force-kill after 5s; respawn. Return 202. Mount POST /v1/workers/:id/restart in build_router. Integration test: restart a worker; GET /v1/workers shows Idle within 30s. cargo test -p anvilml-server --features mock-hardware exits 0.

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
