# Tasks: Phase 015 — Artifact Storage

| Field | Value |
|-------|-------|
| Phase | 015 |
| Name | Artifact Storage |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 14 |

## Overview

Phase 015 implements artifact storage as the next vertical slice. All tasks in this phase build on Phase 14 being complete. Each task implements one module or one concern, with tests, and leaves the binary in a runnable state.

Refer to `docs/ANVILML_DESIGN.md` for the full specification of types, interfaces, and contracts relevant to this phase.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Artifact Storage | P15-A1…P15-A3 | Artifact Storage implementation |

## Prerequisites

Phase 14 complete. Refer to `docs/TASKS_PHASE014.md` for the terminal task and Runnable Proof of Phase 14.

## Task Descriptions

### P15-A1: anvilml-server: artifact/store.rs content-addressed PNG storage

**Context:** Create crates/anvilml-server/src/artifact/store.rs: ArtifactStore{dir:PathBuf,db:SqlitePool}. pub async fn save(&self,job_id:Uuid,image_bytes:&[u8])->Result<ArtifactMeta>: sha256 hash; write to {dir}/{hash}.png; INSERT into artifacts table. pub async fn get(&self,hash:&str)->Result<Option<PathBuf>>. pub async fn list(&self,job_id:Option<Uuid>)->Result<Vec<ArtifactMeta>>. tests: save+get roundtrip,...

**Acceptance criterion:** See context field — all stated commands must exit 0.

---

### P15-A2: anvilml-scheduler: persist ImageReady artifact and update job

**Context:** Extend scheduler.rs event handler: on WorkerEvent::ImageReady{job_id,image_b64,...}: base64 decode; call artifact_store.save(job_id,bytes); broadcast WsEvent::JobImageReady{job_id,artifact_hash,...}. Add artifact_store:Arc<ArtifactStore> to scheduler and AppState. cargo test -p anvilml-scheduler --features mock-hardware exits 0; mock job produces artifact.

**Acceptance criterion:** See context field — all stated commands must exit 0.

---

### P15-A3: anvilml-server: GET /v1/artifacts and GET /v1/artifacts/:hash

**Context:** Create handlers/artifacts.rs: list_artifacts(Query<{job_id:Option<Uuid>}>) returns Vec<ArtifactMeta>. serve_artifact(Path<String>) reads file bytes from ArtifactStore, returns Response with Content-Type image/png. Mount routes in build_router. After completed mock job: curl /v1/artifacts/:hash returns image/png bytes. cargo test -p anvilml-server --features mock-hardware exits 0.

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
