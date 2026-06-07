# Plan Report: P11-A5

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P11-A5                                      |
| Phase       | 011 — Graph Validation                      |
| Description | anvilml-server: POST /v1/jobs validating graph (422 on invalid) |
| Depends on  | P11-A4                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-06-07T09:57:15Z                        |
| Attempt     | 1                                           |

## Objective

Wire `POST /v1/jobs` in `anvilml-server` to accept a `SubmitJobRequest`, call the existing `validate_graph()` from `anvilml-scheduler`, and return either a 422 response (with structured error details) for invalid graphs or a 202 response with a placeholder job ID for valid graphs. Enqueueing is deferred to phase 12.

## Scope

### In Scope
- Create `crates/anvilml-server/src/handlers/jobs.rs` with `submit_job` handler
- Validate graph via `anvilml_scheduler::validate_graph(&req.graph)`
- Return 422 `{error: "invalid_graph", message: <joined errors>, request_id}` on validation failure
- Return 202 `{job_id: <new uuid>, queue_position: 0}` on validation success
- Wire `POST /v1/jobs` route into `build_router()` in `lib.rs`
- Add `pub mod jobs;` to `handlers/mod.rs`
- Unit tests in `jobs.rs`: bad graph → 422, valid ZiT graph → 202

### Out of Scope
- Enqueueing the job (phase 12)
- Persisting job to SQLite (phase 12)
- WebSocket event broadcasting for job queued (phase 12)
- Worker pool dispatch (phase 12)
- Integration tests in `tests/` directory (unit tests in-module are sufficient; integration test file is phase 12)

## Approach

### Step 1: Create `handlers/jobs.rs`

Create new file `crates/anvilml-server/src/handlers/jobs.rs` with the following structure:

**Imports:**
- `axum::{extract::State, extract::Json, http::StatusCode, response::IntoResponse}`
- `serde::{Deserialize, Serialize}`
- `uuid::Uuid` (via anvilml-core re-export — already a transitive dependency through core)
- `anvilml_scheduler::validate_graph`
- `anvilml_core::{SubmitJobRequest, SubmitJobResponse}`

**Handler function:**
```rust
#[utoipa::path(
    post,
    path = "/v1/jobs",
    summary = "Submit a new job for execution",
    request_body = SubmitJobRequest,
    responses(
        (status = 202, description = "Job accepted (placeholder; actual enqueue is phase 12)", body = SubmitJobResponse),
        (status = 422, description = "Invalid graph — validation errors listed", body = ErrorInline)
    )
)]
pub async fn submit_job(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SubmitJobRequest>,
) -> impl IntoResponse {
    let errors = match validate_graph(&req.graph) {
        Ok(_) => return (StatusCode::ACCEPTED, Json(SubmitJobResponse {
            job_id: Uuid::new_v4(),
            queue_position: 0,
        })),
        Err(e) => e,
    };
    let message = errors.join(", ");
    tracing::warn!(errors = %message, "submit_job: graph validation failed");
    (StatusCode::UNPROCESSABLE_ENTITY, Json(serde_json::json!({
        "error": "invalid_graph",
        "message": message,
        "request_id": Uuid::new_v4().to_string(),
    })))
}
```

**Unit tests (in `#[cfg(test)]` module):**
- `submit_job_bad_graph_returns_422`: Send graph with `"type": "NopeNode"` → expect 422, body contains `"invalid_graph"`.
- `submit_job_valid_zit_graph_returns_202`: Send a valid ZiT 5-node graph (ZitLoadPipeline → ZitTextEncode → ZitSampler → ZitDecode → SaveImage with proper edges) → expect 202, body contains `job_id` and `queue_position: 0`.

### Step 2: Register the module in `handlers/mod.rs`

Add one line:
```rust
pub mod jobs;
```

### Step 3: Wire route in `lib.rs`

Add to `build_router()`:
```rust
.route("/v1/jobs", post(handlers::jobs::submit_job))
```

Insert after the `/health` route (first handler group) or before `/v1/events`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-server/src/handlers/jobs.rs` | New handler module with `submit_job`, utoipa annotations, unit tests |
| Modify | `crates/anvilml-server/src/handlers/mod.rs` | Add `pub mod jobs;` |
| Modify | `crates/anvilml-server/src/lib.rs` | Add `.route("/v1/jobs", post(handlers::jobs::submit_job))` to `build_router()` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-server/src/handlers/jobs.rs` | `submit_job_bad_graph_returns_422` | Graph with unknown node type returns 422 with `"error":"invalid_graph"` |
| `crates/anvilml-server/src/handlers/jobs.rs` | `submit_job_valid_zit_graph_returns_202` | Valid ZiT 5-node graph returns 202 with `job_id` and `queue_position: 0` |

## CI Impact

No CI workflow files are modified. The existing CI gates (`cargo test --workspace --features mock-hardware`, `cargo clippy`, format checks) will run as usual. This task only adds new code — no gate changes needed. The OpenAPI drift gate (`cargo run -p anvilml-openapi && git diff --exit-code backend/openapi.json`) may need to be run after implementation since a new endpoint with utoipa annotations is added, but this is noted for the ACT session.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `anvilml-core` re-exports `SubmitJobRequest`/`SubmitJobResponse` not accessible in server crate | Low | Build failure | Verify re-export chain: core `lib.rs` → server uses `use anvilml_core::{SubmitJobRequest, SubmitJobResponse}`; confirmed present in core `lib.rs`. |
| `anvilml-scheduler::validate_graph` requires `&serde_json::Value` but server receives `Json<SubmitJobRequest>` which wraps the graph as `Value` — no conversion needed | None | n/a | Confirmed: `SubmitJobRequest.graph` is already `serde_json::Value`. Direct pass-through. |
| Missing `uuid` dependency in anvilml-server for `Uuid::new_v4()` | Low | Build failure | `anvilml-core` re-exports `Uuid`; server already uses `Uuid` transitively via core types (`SubmitJobResponse.job_id: Uuid`). No direct dep needed. |
| utoipa annotations cause OpenAPI drift gate failure | Medium | CI gate fails on first run | Document as expected; ACT session runs OpenAPI regeneration and checks diff. Not a plan defect. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server --features mock-hardware` exits 0 (new unit tests pass)
- [ ] `curl -X POST http://127.0.0.1:8488/v1/jobs -d '{"graph":{"nodes":[{"id":"n0","type":"NopeNode","inputs":{}}]}}'` returns HTTP 422 with body containing `"error":"invalid_graph"`
- [ ] A valid ZiT 5-node graph submitted to `POST /v1/jobs` returns HTTP 202 with `job_id` (UUID) and `queue_position: 0`
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
