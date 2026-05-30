# Tasks: Phase 007 — HTTP & WebSocket Server

| Field            | Value                                                                       |
|------------------|-----------------------------------------------------------------------------|
| Phase            | 007                                                                         |
| Name             | HTTP & WebSocket Server                                                     |
| ANVIL Milestone  | M4 (part 1)                                                                 |
| Status           | Draft                                                                       |
| Depends on phases| 1, 2, 3, 4, 5, 6                                                            |
| Task file        | `forge/tasks/tasks_phase007.json`                                           |
| Design reference | `ANVILML_DESIGN.md` §10 (Server), §11 (Frontend), §12 (Artifacts), §17 (OpenAPI), §18 (Error Model) |

---

## Overview

Phase 007 implements the `anvilml-server` crate: the axum HTTP/WebSocket server that forms the sole external integration surface of AnvilML. By the end of this phase every REST endpoint and the WebSocket event stream specified in `ANVILML_DESIGN.md §10.3–10.4` is implemented, tested with an in-process test client, and documented by a generated `openapi.json`.

The server is decomposed into seven tasks following the natural dependency chain within the crate: AppState and middleware first (P7-A1), then handlers in order of complexity (system, jobs, models/workers, artifacts), then the WebSocket handler and frontend serving last (P7-B1, P7-B2). The OpenAPI generator is also completed in P7-B2 because the handler annotations it reflects are all present by that point.

The error model (`ANVILML_DESIGN.md §18`) must be consistently applied across every handler: all 4xx/5xx responses use the uniform `{ "error": "code", "message": "...", "request_id": "..." }` JSON body. There are no HTML error pages. The `X-Request-Id` header value is echoed in the error body via the `SetRequestIdLayer` middleware.

At the end of this phase: `cargo test -p anvilml-server --features mock-hardware` passes for all handler groups; `cargo run -p anvilml-openapi` generates a non-empty `backend/openapi.json` and `git diff --exit-code` passes.

---

## Group Reference

| Group | Subsystem        | Tasks          | Summary                                                        |
|-------|------------------|----------------|----------------------------------------------------------------|
| A     | anvilml-server   | P7-A1 … P7-A5  | AppState, system/jobs/models/workers/artifacts handlers        |
| B     | anvilml-server   | P7-B1, P7-B2   | WebSocket events, frontend serving, OpenAPI generator          |

---

## Prerequisites

- P6-A4 complete: `JobScheduler` with `submit`, `cancel`, and `start_dispatch_loop` is implemented.
- `ArtifactStore` API (`save`, `get_path`, `list`, `delete_for_job`) is defined (it is implemented in this phase in P7-A5; the interface needs to be agreed with P6-A4's dispatch loop usage of `save`).
- `EventBroadcaster` wraps a `broadcast::Sender<Arc<WsEvent>>` — defined in this phase in P7-A1 but referenced by the scheduler in P6-A4. Resolve the circular reference by defining `EventBroadcaster` in `anvilml-core` or accepting it as a forward dependency.

---

## Contract Documents Applicable to This Phase

| Document section           | Relevant tasks        | What must match                                                        |
|----------------------------|-----------------------|------------------------------------------------------------------------|
| `ANVILML_DESIGN.md` §10.1  | P7-A1                 | `AppState` fields exactly as specified                                 |
| `ANVILML_DESIGN.md` §10.2  | P7-A1                 | Middleware stack order (outermost first)                               |
| `ANVILML_DESIGN.md` §10.3  | P7-A2 … P7-A5         | All routes: method, path, success status code, response body           |
| `ANVILML_DESIGN.md` §10.4  | P7-B1                 | WS path, JSON text frames, ping 30 s, disconnect on lag (close 1008)   |
| `ANVILML_DESIGN.md` §10.5  | P7-A2                 | `system.stats` tick interval 5 s                                       |
| `ANVILML_DESIGN.md` §11    | P7-B2                 | Local/Remote/Headless frontend modes, SPA fallback, missing-dir warning|
| `ANVILML_DESIGN.md` §12    | P7-A5                 | Artifact save pipeline, two-char prefix sharding, cache headers        |
| `ANVILML_DESIGN.md` §18    | All                   | Uniform error body; `request_id` from `X-Request-Id`                  |

---

## Task Descriptions

### Group A — Core Handlers

#### P7-A1: anvilml-server — AppState, EventBroadcaster, middleware stack

**Goal:** Establish the shared application state struct, the WebSocket event broadcaster, and the axum router skeleton with its full middleware stack.

**Files to create or modify:**
- `crates/anvilml-server/src/state.rs` — `AppState`
- `crates/anvilml-server/src/ws/broadcaster.rs` — `EventBroadcaster`
- `crates/anvilml-server/src/lib.rs` — router constructor with middleware
- `crates/anvilml-server/Cargo.toml` — add `axum`, `tower`, `tower-http` (compression, trace, cors, request-id), `tracing`, `tracing-subscriber`, `tokio` (full)

**Key implementation notes:**
- `AppState` exactly as in `ANVILML_DESIGN.md §10.1`. All fields are `Arc`-wrapped so `AppState` is cheaply cloneable for use as axum state.
- `EventBroadcaster::new(capacity: usize) -> Self`: create `broadcast::channel(capacity)`. `send(&self, event: Arc<WsEvent>)`: call `sender.send(event)`, ignore `SendError` (no subscribers is not an error).
- Middleware order (outermost first): `TraceLayer`, `SetRequestIdLayer` (generates UUID v4 request IDs), `CompressionLayer`, `CorsLayer::very_permissive()` (for MVP localhost use).
- The router skeleton must compile with stub handlers returning `StatusCode::NOT_IMPLEMENTED` for all routes. Routes are filled in by subsequent tasks.

**Acceptance criterion:** `cargo build -p anvilml-server --features mock-hardware` exits 0.

---

#### P7-A2: anvilml-server — health, system, env handlers and system.stats tick

**Goal:** Implement the three system-information endpoints and the background task that broadcasts periodic VRAM+RAM statistics.

**Files to create or modify:**
- `crates/anvilml-server/src/handlers/system.rs`

**Key implementation notes:**
- `GET /health` → `200 { "status": "ok", "version": env!("CARGO_PKG_VERSION"), "uptime_s": u64 }`. Track start time in `AppState` as a `std::time::Instant`.
- `GET /v1/system` → `200 HardwareInfo` from `AppState.hardware.read()`.
- `GET /v1/system/env` → `200 EnvReport` from `AppState.env_report.read()`.
- `system.stats` background task (started by `lib.rs`): `tokio::time::interval(Duration::from_secs(5))`. On each tick: iterate `AppState.workers.list()`, collect `vram_used_mib` per device, read `sysinfo::System` for current host RAM. Build `SystemStatsEvent { gpus: [...], ram_used_mib, ram_total_mib }`. Call `AppState.broadcaster.send(Arc::new(WsEvent::SystemStats(...)))`.

**Acceptance criterion:** `cargo test -p anvilml-server --features mock-hardware -- system` exits 0.

---

#### P7-A3: anvilml-server — job handlers

**Goal:** Implement all six job-related REST endpoints with correct status codes, error codes, and cursor pagination.

**Files to create or modify:**
- `crates/anvilml-server/src/handlers/jobs.rs`

**Key implementation notes:**
- `POST /v1/jobs`: deserialize `SubmitJobRequest`; call `scheduler.submit(req)`; on `Err(InvalidGraph(...))` → `422 { "error": "invalid_graph", ... }`; on success → `202 SubmitJobResponse`.
- `GET /v1/jobs`: accept `?status=`, `?limit=` (default 100, max 1000), `?before=<iso8601>`. Query DB with filters. Return `200 Vec<Job>`.
- `GET /v1/jobs/:id`: query DB by UUID string; `404 not_found` if absent.
- `POST /v1/jobs/:id/cancel`: call `scheduler.cancel(id)`; map `AnvilError::JobNotFound` → 404, `job_not_cancellable` → 409, success → 202.
- `DELETE /v1/jobs/:id`: read from DB; if `Running` or `Queued` → `409 job_active`; else delete job row + call `artifact_store.delete_for_job(id)` → 204.
- `DELETE /v1/jobs`: accept `?status=completed|failed|cancelled|all`. Bulk delete matching terminal jobs + artifacts. Return `200 { "removed": u32 }`.
- All error responses must include `"request_id"` extracted from the request's `X-Request-Id` header (set by middleware).

**Acceptance criterion:** `cargo test -p anvilml-server --features mock-hardware -- jobs` exits 0 covering all 404/409/422 branches.

---

#### P7-A4: anvilml-server — model and worker handlers

**Goal:** Implement the five model and worker REST endpoints.

**Files to create or modify:**
- `crates/anvilml-server/src/handlers/models.rs`
- `crates/anvilml-server/src/handlers/workers.rs`

**Key implementation notes:**
- `GET /v1/models`: accept `?kind=`. Call `registry.list(kind)`. Return `200 Vec<ModelMeta>`.
- `GET /v1/models/:id`: call `registry.get(id)`; 404 on `None`.
- `POST /v1/models/rescan`: spawn `tokio::spawn(async { registry.rescan(&cfg.model_dirs).await })` without waiting; return `202` immediately.
- `GET /v1/workers`: call `workers.list()`. Return `200 Vec<WorkerInfo>`.
- `POST /v1/workers/:id/restart`: call `workers.restart(id)`; 404 if worker_id not found; 202 on success.

**Acceptance criterion:** `cargo test -p anvilml-server --features mock-hardware -- models` and `-- workers` both exit 0.

---

#### P7-A5: anvilml-server — ArtifactStore and artifact handlers

**Goal:** Implement the `ArtifactStore` that saves artifact files and the two artifact REST endpoints.

**Files to create or modify:**
- `crates/anvilml-server/src/artifact/store.rs` — `ArtifactStore`
- `crates/anvilml-server/src/handlers/artifacts.rs`
- `crates/anvilml-server/Cargo.toml` — add `sha2`, `hex`, `base64`, `tokio` (fs)

**Key implementation notes:**
- `ArtifactStore::save(job_id, image_b64, meta_input) -> Result<ArtifactMeta>`:
  1. `base64::decode(image_b64)` → `png_bytes: Vec<u8>`.
  2. `hash = hex::encode(Sha256::digest(&png_bytes))`.
  3. `artifact_path = artifact_dir / &hash[0..2] / format!("{hash}.png")`.
  4. `tokio::fs::create_dir_all(artifact_path.parent())` then `tokio::fs::write(&artifact_path, &png_bytes)`.
  5. Insert `ArtifactMeta` into DB. Increment `jobs.artifact_count`. Return `ArtifactMeta`.
- `GET /v1/artifacts`: accept `?job_id=`. Query DB. Return `200 Vec<ArtifactMeta>`.
- `GET /v1/artifacts/:hash`: call `artifact_store.get_path(hash)`; 404 if absent; serve the file with `Content-Type: image/png`, `Cache-Control: public, immutable, max-age=31536000`, `ETag: "{hash}"`. Use axum's `tokio::fs::File`-based response.

**Acceptance criterion:** `cargo test -p anvilml-server --features mock-hardware -- artifacts` exits 0.

---

### Group B — WebSocket and Frontend

#### P7-B1: anvilml-server — WebSocket /v1/events handler

**Goal:** Implement the WebSocket upgrade endpoint that streams `WsEvent` JSON frames to subscribers and disconnects lagging clients.

**Files to create or modify:**
- `crates/anvilml-server/src/ws/handler.rs`
- `crates/anvilml-server/Cargo.toml` — add `tokio-tungstenite` (dev-dep for tests)

**Key implementation notes:**
- `GET /v1/events`: `axum::extract::WebSocketUpgrade` → call `ws.on_upgrade(handle_socket)`.
- `handle_socket(socket, broadcaster)`: subscribe `broadcaster.subscribe()` → `Receiver<Arc<WsEvent>>`. Spawn two tasks:
  - *Send task*: loop on `receiver.recv()`. On `Ok(event)`: serialize to JSON string, send `Message::Text(json)`. On `Err(RecvError::Lagged(_))`: send `Message::Close(Some(CloseFrame { code: 1008, reason: "buffer_overflow".into() }))` and return.
  - *Ping task*: `tokio::time::interval(30s)` → send `Message::Ping(vec![])`. On send error: return (client disconnected).
- Never replay history. Do not send any backfill on connect.
- Write tests using `tokio-tungstenite` connecting to an in-process axum server: (1) connect and receive a `system.stats` frame within 6 s; (2) simulate a lagged client by filling the broadcast channel past capacity and assert the connection is closed with code 1008.

**Acceptance criterion:** `cargo test -p anvilml-server --features mock-hardware -- ws` exits 0.

---

#### P7-B2: anvilml-server — frontend serving and OpenAPI generator

**Goal:** Implement the three frontend serving modes as the lowest-priority catch-all route, and complete the `anvilml-openapi` binary that generates `backend/openapi.json` from `utoipa` annotations.

**Files to create or modify:**
- `crates/anvilml-server/src/frontend.rs` — `add_frontend_route(router, mode) -> Router`
- `crates/anvilml-openapi/src/main.rs` — full `utoipa::OpenApi` implementation
- `crates/anvilml-server/Cargo.toml` — add `tower-http` (ServeDir feature), `hyper` (for Remote proxy)

**Key implementation notes:**
- **`Local { path }`**: use `tower_http::services::ServeDir::new(path).fallback(ServeFile::new(path.join("index.html")))`. If `path` does not exist at router build time, log a warning and substitute a catch-all handler that returns a minimal inline HTML page: `<h1>AnvilML</h1><p>Frontend not found at ./bloomery. API available at /v1/.</p>`.
- **`Remote { url }`**: a catch-all `GET /*` axum handler that creates a `hyper` client request to `{url}{original_path}`, forwards headers (strip hop-by-hop), rewrites `Host`, and streams the response back. This is for development use only; error tolerance can be minimal.
- **`Headless`**: register no catch-all route; the axum router returns 404 for all non-API paths.
- All frontend routes must be registered after all `/v1/*` and `/health` routes so they do not shadow API paths.
- `anvilml-openapi/src/main.rs`: derive `#[derive(OpenApi)]` on a struct referencing all handler functions (annotated with `#[utoipa::path(...)]`) and all `anvilml-core` types (annotated with `#[derive(utoipa::ToSchema)]`). Write the resulting JSON to `backend/openapi.json`. Run `git diff --exit-code backend/openapi.json` in CI after this binary runs; if it changes, CI fails.

**Acceptance criterion:** `cargo run -p anvilml-openapi` exits 0 and produces a non-empty `backend/openapi.json` with all routes present.

---

## Phase Acceptance Criteria

```
cargo test -p anvilml-server --features mock-hardware
cargo run -p anvilml-openapi
git diff --exit-code backend/openapi.json
cargo clippy --workspace --features mock-hardware -- -D warnings
```

---

## Known Constraints and Gotchas

- The `ArtifactStore` is used by both `anvilml-server` (this phase) and `anvilml-scheduler` (phase 006, in the dispatch loop's `ImageReady` handler). Resolve this by defining `ArtifactStore` in `anvilml-server` and passing it to the scheduler via `AppState`, or by moving `ArtifactStore` to a shared module. The cleanest approach for MVP is to define it in `anvilml-server` and have the scheduler call it via the `AppState.artifact_store` reference — meaning the scheduler depends on the server, which is already the case in the crate dependency graph (§2.1).
- The `EventBroadcaster` is constructed once and shared via `Arc`. Its `broadcast::channel` capacity must be set to `cfg.limits.ws_broadcast_capacity` (default 256). A subscriber that is behind by more than 256 events receives `RecvError::Lagged` and must be disconnected immediately.
- `ServeDir` on Windows uses `PathBuf` for path resolution, which normalises separators. This works correctly on Windows as long as the `path` in `FrontendMode::Local` is stored as a `PathBuf` (not a raw string with forward slashes).
- The OpenAPI generator binary must be run after `cargo build` because it uses Rust's type system to derive schemas. It is not a code generator that runs before compilation — it compiles and runs against the final types. The `git diff --exit-code` CI check therefore must come after the `cargo run -p anvilml-openapi` step.
