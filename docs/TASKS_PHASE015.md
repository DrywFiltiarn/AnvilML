# Tasks: Phase 015 — Artifact Storage

| Field | Value |
|-------|-------|
| Phase | 015 |
| Name | Artifact Storage |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 14 |

## Overview

Phase 015 adds content-addressed artifact storage. When a worker emits `WorkerEvent::ImageReady`, the base64-encoded PNG is decoded, hashed with SHA-256, written to disk as `{artifacts_dir}/{hash}.png`, and recorded in the `artifacts` SQLite table. Clients can then list artifacts (optionally filtered by job ID) and retrieve the raw PNG bytes via `GET /v1/artifacts/:hash`.

At phase start, `ImageReady` events are received by the scheduler but not persisted. At phase end, every completed mock job produces at least one artifact retrievable as `image/png` via HTTP.

Phase 016 (Live Job Events) depends on the `WsEvent::JobImageReady` broadcast added here to relay artifact hashes to WebSocket clients in real time.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | anvilml-server + scheduler | P15-A1 … P15-A3 | ArtifactStore, ImageReady persistence, GET /v1/artifacts endpoints |

## Prerequisites

Phase 014 complete. Mock jobs produce `WorkerEvent::ImageReady { job_id, image_b64, width, height }`. `JobScheduler` subscribes to the worker event broadcast. `AppState` can be extended with `Arc<ArtifactStore>`. The `artifacts` SQLite table schema is defined in `ANVILML_DESIGN.md §14`.

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|-------------------|-------------------|-----------------|
| `ANVILML_DESIGN.md §8.2` | P15-A2 | `WorkerEvent::ImageReady` field names: `job_id`, `image_b64`, `width`, `height` |
| `ANVILML_DESIGN.md §5.5` | P15-A1 | `ArtifactMeta` fields: `hash`, `job_id`, `created_at`, `width`, `height`, `size_bytes` |
| `ANVILML_DESIGN.md §5.8` | P15-A2 | `WsEvent::JobImageReady { job_id, artifact_hash, width, height }` |
| `ANVILML_DESIGN.md §12.4` | P15-A3 | GET /v1/artifacts and GET /v1/artifacts/:hash response shapes |

## Task Descriptions

### Group A — anvilml-server and anvilml-scheduler

#### P15-A1: anvilml-server: artifact/store.rs content-addressed PNG storage

**Goal:** Create `ArtifactStore` in `crates/anvilml-server/src/artifact/store.rs` — the persistence layer for PNG artifacts. SHA-256 hashing ensures each unique image is stored exactly once regardless of how many jobs produce it.

**Files to create or modify:**
- `crates/anvilml-server/src/artifact/store.rs` — new file; `ArtifactStore` struct and all methods
- `crates/anvilml-server/src/artifact/mod.rs` — new file; `pub mod store; pub use store::ArtifactStore`
- `crates/anvilml-server/src/lib.rs` — add `pub mod artifact`

**Key implementation notes:**
- `ArtifactStore { dir: PathBuf, db: SqlitePool }`
- `pub async fn save(&self, job_id: Uuid, image_bytes: &[u8]) -> Result<ArtifactMeta>`: compute SHA-256 hex digest; write bytes to `{dir}/{hash}.png` (skip write if file exists — idempotent); `INSERT OR IGNORE INTO artifacts ...`; return `ArtifactMeta`
- `pub async fn get(&self, hash: &str) -> Result<Option<PathBuf>>` — returns path if file exists on disk
- `pub async fn list(&self, job_id: Option<Uuid>) -> Result<Vec<ArtifactMeta>>` — queries `artifacts` table

**Acceptance criterion:** `cargo test -p anvilml-server --features mock-hardware` exits 0 with ≥ 4 tests (save + get roundtrip; hash is deterministic for same bytes; list returns saved artifact; save is idempotent).

---

#### P15-A2: anvilml-scheduler: persist ImageReady artifact and update job

**Goal:** Wire `ArtifactStore` into the scheduler's worker event handler so `WorkerEvent::ImageReady` triggers artifact persistence and the `WsEvent::JobImageReady` broadcast.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/scheduler.rs` — extend Completed/Failed event handler to handle `ImageReady`
- `crates/anvilml-server/src/state.rs` — add `artifact_store: Arc<ArtifactStore>` to `AppState`

**Key implementation notes:**
- On `WorkerEvent::ImageReady { job_id, image_b64, width, height }`: base64-decode to `Vec<u8>`; call `artifact_store.save(job_id, &bytes).await`; broadcast `WsEvent::JobImageReady { job_id, artifact_hash: meta.hash, width, height }`
- `Arc<ArtifactStore>` is passed into `JobScheduler` constructor and stored as a field
- `tracing::info!(job_id, artifact_hash, size_bytes, "artifact saved")`

**Acceptance criterion:** `cargo test -p anvilml-scheduler --features mock-hardware` exits 0; integration test submits mock job and asserts `ArtifactStore::list(Some(job_id))` returns ≥ 1 entry after Completed.

---

#### P15-A3: anvilml-server: GET /v1/artifacts and GET /v1/artifacts/:hash

**Goal:** Expose the two artifact retrieval endpoints. `GET /v1/artifacts` returns metadata for all artifacts (optionally filtered by job ID). `GET /v1/artifacts/:hash` streams the raw PNG bytes with `Content-Type: image/png`.

**Files to create or modify:**
- `crates/anvilml-server/src/handlers/artifacts.rs` — new file; `list_artifacts` and `serve_artifact` handlers
- `crates/anvilml-server/src/lib.rs` — mount both routes in `build_router`

**Key implementation notes:**
- `list_artifacts(State<AppState>, Query<{ job_id: Option<Uuid> }>) -> Result<Json<Vec<ArtifactMeta>>, AnvilError>`
- `serve_artifact(State<AppState>, Path<String>) -> Result<Response, AnvilError>`: call `artifact_store.get(hash)`; if `None` return `AnvilError::ArtifactNotFound` (404); read file bytes; return `Response` with `Content-Type: image/png` header
- Integration test: after completed mock job, `GET /v1/artifacts/:hash` returns 200 with non-empty body and correct content type

**Acceptance criterion:** `cargo test -p anvilml-server --features mock-hardware` exits 0; test verifies `image/png` content type and non-empty PNG body on `serve_artifact`.

---

## Phase Acceptance Criteria

```bash
cargo test --workspace --features mock-hardware
ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
```

## Known Constraints and Gotchas

- `ArtifactStore::save` must be idempotent: if the same hash is submitted twice (e.g. two jobs producing the same image), the second write is a no-op and the INSERT uses `INSERT OR IGNORE`.
- The `artifacts_dir` path must be created on `ArtifactStore::new` if it does not exist (`std::fs::create_dir_all`).
- Follow `FORGE_AGENT_RULES.md §12` for all inline documentation.
- Follow `FORGE_AGENT_RULES.md §11` for all logging.
- Test isolation: every test that sets env vars must restore them unconditionally per `ENVIRONMENT.md §11.3`.
