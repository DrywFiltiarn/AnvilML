# Plan Report: P18-F1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P18-F1                                            |
| Phase       | 18 — HTTP/WebSocket Server Completion             |
| Description | anvilml-openapi: real OpenAPI generation from utoipa annotations |
| Depends on  | P18-C3, P18-D2, P18-E2                            |
| Project     | anvilml                                           |
| Planned at  | 2026-07-12T15:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Replace Phase 1's stub print in `crates/anvilml-openapi/src/main.rs` with real OpenAPI spec generation. Add `#[utoipa::path(...)]` annotations to every handler function across all seven handler files (`health`, `system`, `jobs`, `models`, `workers`, `artifacts`, `nodes`), derive `utoipa::OpenApi` on a struct listing every path and every `ToSchema`-deriving response type, then call `ApiDoc::openapi().to_pretty_json()` and write to `api/openapi.json`. This is the first time `api/openapi.json` has real content — the `openapi-drift` CI gate (a Phase 1 placeholder) becomes meaningful from this task onward. Acceptance: `cargo run -p anvilml-openapi` exits 0, produces a valid non-empty `api/openapi.json` with every §13.4 route; a second run produces an identical file (idempotent).

## Scope

### In Scope
- Add `utoipa` crate (v5.5.0) with `macros`, `axum_extras`, `uuid`, and `serde_json` features to `crates/anvilml-server/Cargo.toml` and `crates/anvilml-openapi/Cargo.toml`.
- Derive `ToSchema` on every response-type struct in `anvilml-server` that is not already `ToSchema`: `HealthResponse`, `ComponentVersions`, `SubmitJobResponse`, `ListJobsParams`, `BulkClearParams`, `RemovedCount`, `ListModelsParams`, `ListArtifactsParams`.
- Add `#[utoipa::path(...)]` annotations to every handler function across all seven handler files:
  - `health.rs`: `health` (GET /health)
  - `system.rs`: `get_system` (GET /v1/system), `get_system_env` (GET /v1/system/env), `get_system_versions` (GET /v1/system/versions)
  - `jobs.rs`: `submit_job` (POST /v1/jobs), `list_jobs` (GET /v1/jobs), `get_job` (GET /v1/jobs/{id}), `cancel_job` (POST /v1/jobs/{id}/cancel), `delete_job` (DELETE /v1/jobs/{id}), `bulk_clear_jobs` (DELETE /v1/jobs)
  - `models.rs`: `list_models` (GET /v1/models), `get_model` (GET /v1/models/{id}), `rescan_models` (POST /v1/models/rescan)
  - `workers.rs`: `list_workers` (GET /v1/workers), `restart_worker` (POST /v1/workers/{id}/restart)
  - `artifacts.rs`: `list_artifacts` (GET /v1/artifacts), `get_artifact` (GET /v1/artifacts/{hash})
  - `nodes.rs`: `list_nodes` (GET /v1/nodes)
- Create `crates/anvilml-server/src/openapi.rs` with the `ApiDoc` struct that derives `utoipa::OpenApi`, listing every path and every component schema.
- Update `crates/anvilml-openapi/src/main.rs` to call `ApiDoc::openapi().to_pretty_json()` and write to `api/openapi.json`.
- Bump `anvilml-server` patch version (0.1.26 → 0.1.27) and `anvilml-openapi` patch version per §12 of ENVIRONMENT.md.

### Out of Scope
None. `defers_to (from JSON): []` — this task has no deferrals. The `openapi-drift` CI job wiring (P18-F2) is a separate task in the same phase but is not part of this task's scope; this task only produces the real `api/openapi.json` content.

## Existing Codebase Assessment

The codebase already has `utoipa::ToSchema` derived on all domain types in `anvilml-core` (types in `types/job.rs`, `types/model.rs`, `types/hardware.rs`, `types/worker.rs`, `types/artifact.rs`, `types/node.rs`, `types/events.rs`). These are the response body types referenced by the handler functions.

Handler functions in `crates/anvilml-server/src/handlers/*.rs` are thin delegations with zero business logic, as specified by `ANVILML_DESIGN.md §3.3`. They already use `#[tracing::instrument]` or `#[instrument]` for logging. Response types in the server crate (`HealthResponse`, `ComponentVersions`, `SubmitJobResponse`, `ListJobsParams`, `BulkClearParams`, `RemovedCount`, `ListModelsParams`, `ListArtifactsParams`) are `serde::Serialize` but do not yet derive `ToSchema`.

The `anvilml-openapi` binary at `crates/anvilml-openapi/src/main.rs` is a 3-line stub that only prints `"openapi generation stub"`. Its `Cargo.toml` already depends on `anvilml-core` and `anvilml-server` via path dependencies, so it can import the server's types.

The `api/` directory exists with only a `.gitkeep` file — `api/openapi.json` has never been generated.

Error responses use `AnvilError`'s `IntoResponse` impl which produces a structured JSON body (`ErrorBody`). `AnvilError` itself does not derive `ToSchema` (it's an error type, not a response body type), so error responses will be documented inline in the `#[utoipa::path]` annotations using `status` and `description` attributes.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | utoipa  | 5.5.0           | rust-docs MCP  | macros, axum_extras, uuid, serde_json |

The `utoipa` 5.5.0 MSRV is 1.75, compatible with this project's pinned 1.96.0. The `macros` feature is default and provides `#[derive(OpenApi)]` and `#[utoipa::path]`. The `axum_extras` feature enables axum-specific response type resolution. The `uuid` feature provides OpenAPI type mapping for `uuid::Uuid`. The `serde_json` feature enables `JsonSchema` for `serde_json::Value` (used in `SubmitJobRequest.graph`).

## Approach

### Step 1: Add utoipa dependency to both crates

Add `utoipa = { version = "5.5.0", features = ["macros", "axum_extras", "uuid", "serde_json"] }` to:
- `crates/anvilml-server/Cargo.toml` (under `[dependencies]`)
- `crates/anvilml-openapi/Cargo.toml` (under `[dependencies]`)

### Step 2: Derive ToSchema on response types in anvilml-server

Add `use utoipa::ToSchema;` and derive `ToSchema` on these response-type structs:

**`crates/anvilml-server/src/handlers/health.rs`**:
- `HealthResponse` — add `#[derive(ToSchema)]` to existing derive block.

**`crates/anvilml-server/src/handlers/system.rs`**:
- `ComponentVersions` — add `#[derive(ToSchema)]` to existing derive block.

**`crates/anvilml-server/src/handlers/jobs.rs`**:
- `SubmitJobRequest` — add `#[derive(ToSchema)]` (it's the request body).
- `SubmitJobResponse` — add `#[derive(ToSchema)]`.
- `ListJobsParams` — add `#[derive(ToSchema)]` (query params; also implement `utoipa::IntoParams`).
- `BulkClearParams` — add `#[derive(ToSchema)]` (query params; also implement `utoipa::IntoParams`).
- `RemovedCount` — add `#[derive(ToSchema)]`.

**`crates/anvilml-server/src/handlers/models.rs`**:
- `ListModelsParams` — add `#[derive(ToSchema)]` (query params; also implement `utoipa::IntoParams`).

**`crates/anvilml-server/src/handlers/workers.rs`**:
- No new response types needed — `WorkerInfo` is already `ToSchema` from `anvilml-core`.

**`crates/anvilml-server/src/handlers/artifacts.rs`**:
- `ListArtifactsParams` — add `#[derive(ToSchema)]` (query params; also implement `utoipa::IntoParams`).

**Rationale on IntoParams for query types:** The `utoipa::IntoParams` trait (enabled by the `macros` feature) lets us document query parameters inline on the struct rather than repeating them in each `#[utoipa::path]` annotation. This is the idiomatic utoipa pattern for parameterized requests.

### Step 3: Add #[utoipa::path(...)] annotations to every handler

For each handler function, add a `#[utoipa::path(...)]` attribute before the function signature. Each annotation specifies:
- `operation_id`: a unique snake_case identifier per route+method combination
- `tags`: the handler module name (e.g., `"Health"`, `"System"`, `"Jobs"`, `"Models"`, `"Workers"`, `"Artifacts"`, `"Nodes"`)
- `summary`: a one-line description
- `description`: a longer description referencing the §13.4 route table
- `responses`: the success response (200/202/204 with the response type) and error responses (400/404/409/413/500/503 with inline `ref` to `AnvilError` body shape)
- `params`: query/path parameters where applicable, using the `IntoParams` types

The specific annotations per file:

**`health.rs`** — `health`:
- GET `/health`, operation_id: `health_check`, tags: `["Health"]`
- Response: 200 → `HealthResponse`
- Error responses: none (health never fails)

**`system.rs`** — `get_system`:
- GET `/v1/system`, operation_id: `get_system`, tags: `["System"]`
- Response: 200 → `HardwareInfo` (already `ToSchema` from core)

**`system.rs`** — `get_system_env`:
- GET `/v1/system/env`, operation_id: `get_system_env`, tags: `["System"]`
- Response: 200 → `EnvReport` (already `ToSchema` from core)

**`system.rs`** — `get_system_versions`:
- GET `/v1/system/versions`, operation_id: `get_system_versions`, tags: `["System"]`
- Response: 200 → `ComponentVersions` (newly `ToSchema` in this task)

**`jobs.rs`** — `submit_job`:
- POST `/v1/jobs`, operation_id: `submit_job`, tags: `["Jobs"]`
- Request body: `SubmitJobRequest` (newly `ToSchema`)
- Response: 202 → `SubmitJobResponse` (newly `ToSchema`)

**`jobs.rs`** — `list_jobs`:
- GET `/v1/jobs`, operation_id: `list_jobs`, tags: `["Jobs"]`
- Params: `ListJobsParams` (newly `IntoParams`)
- Response: 200 → `Vec<Job>` (Job already `ToSchema` from core)

**`jobs.rs`** — `get_job`:
- GET `/v1/jobs/{id}`, operation_id: `get_job`, tags: `["Jobs"]`
- Path param: `id: Uuid`
- Response: 200 → `Job`

**`jobs.rs`** — `cancel_job`:
- POST `/v1/jobs/{id}/cancel`, operation_id: `cancel_job`, tags: `["Jobs"]`
- Path param: `id: Uuid`
- Response: 202 (no body), 409 (no body), 404 → error

**`jobs.rs`** — `delete_job`:
- DELETE `/v1/jobs/{id}`, operation_id: `delete_job`, tags: `["Jobs"]`
- Path param: `id: Uuid`
- Response: 204 (no body), 409 (no body), 404 → error

**`jobs.rs`** — `bulk_clear_jobs`:
- DELETE `/v1/jobs`, operation_id: `bulk_clear_jobs`, tags: `["Jobs"]`
- Params: `BulkClearParams` (newly `IntoParams`)
- Response: 200 → `RemovedCount` (newly `ToSchema`)

**`models.rs`** — `list_models`:
- GET `/v1/models`, operation_id: `list_models`, tags: `["Models"]`
- Params: `ListModelsParams` (newly `IntoParams`)
- Response: 200 → `Vec<ModelMeta>` (ModelMeta already `ToSchema` from core)

**`models.rs`** — `get_model`:
- GET `/v1/models/{id}`, operation_id: `get_model`, tags: `["Models"]`
- Path param: `model_id: String`
- Response: 200 → `ModelMeta`

**`models.rs`** — `rescan_models`:
- POST `/v1/models/rescan`, operation_id: `rescan_models`, tags: `["Models"]`
- Response: 202 (no body)

**`workers.rs`** — `list_workers`:
- GET `/v1/workers`, operation_id: `list_workers`, tags: `["Workers"]`
- Response: 200 → `Vec<WorkerInfo>` (WorkerInfo already `ToSchema` from core)

**`workers.rs`** — `restart_worker`:
- POST `/v1/workers/{id}/restart`, operation_id: `restart_worker`, tags: `["Workers"]`
- Path param: `id: String`
- Response: 202 (no body), 409 (no body), 404 → error

**`artifacts.rs`** — `list_artifacts`:
- GET `/v1/artifacts`, operation_id: `list_artifacts`, tags: `["Artifacts"]`
- Params: `ListArtifactsParams` (newly `IntoParams`)
- Response: 200 → `Vec<ArtifactMeta>` (ArtifactMeta already `ToSchema` from core)

**`artifacts.rs`** — `get_artifact`:
- GET `/v1/artifacts/{hash}`, operation_id: `get_artifact`, tags: `["Artifacts"]`
- Path param: `hash: String`
- Response: 200 → binary (documented as `description = "Raw PNG bytes"` with `content_type = "image/png"`)

**`nodes.rs`** — `list_nodes`:
- GET `/v1/nodes`, operation_id: `list_nodes`, tags: `["Nodes"]`
- Response: 200 → `Vec<NodeTypeDescriptor>` (NodeTypeDescriptor already `ToSchema` from core)

For error responses on every endpoint, document the `AnvilError` body shape inline using `response = ...` with a description that references the JSON structure from `ANVILML_DESIGN.md §13.5`: `{ "error": "...", "message": "...", "request_id": "uuid" }`.

### Step 4: Create openapi.rs with the ApiDoc struct

Create `crates/anvilml-server/src/openapi.rs`:

```rust
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        health::health,
        system::get_system,
        system::get_system_env,
        system::get_system_versions,
        jobs::submit_job,
        jobs::list_jobs,
        jobs::get_job,
        jobs::cancel_job,
        jobs::delete_job,
        jobs::bulk_clear_jobs,
        models::list_models,
        models::get_model,
        models::rescan_models,
        workers::list_workers,
        workers::restart_worker,
        artifacts::list_artifacts,
        artifacts::get_artifact,
        nodes::list_nodes,
    ),
    components(
        schemas(
            HealthResponse,
            ComponentVersions,
            SubmitJobRequest,
            SubmitJobResponse,
            ListJobsParams,
            BulkClearParams,
            RemovedCount,
            ListModelsParams,
            ListArtifactsParams,
            Job,
            ModelMeta,
            HardwareInfo,
            EnvReport,
            WorkerInfo,
            ArtifactMeta,
            NodeTypeDescriptor,
            JobStatus,
            ModelKind,
            ModelDtype,
            ModelFormat,
            SlotType,
            WorkerStatus,
            InferenceCaps,
            CapabilitySource,
            GpuDevice,
            HardwareInfo,
        )
    ),
    tags(
        (name = "Health", description = "Health check endpoints"),
        (name = "System", description = "System information endpoints"),
        (name = "Jobs", description = "Job management endpoints"),
        (name = "Models", description = "Model registry endpoints"),
        (name = "Workers", description = "Worker management endpoints"),
        (name = "Artifacts", description = "Artifact storage endpoints"),
        (name = "Nodes", description = "Node type registry endpoints"),
    ),
)]
pub struct ApiDoc;
```

The `components(schemas(...))` section lists every type that appears as a response body or parameter type. Types already `ToSchema` from `anvilml-core` (Job, ModelMeta, HardwareInfo, EnvReport, WorkerInfo, ArtifactMeta, NodeTypeDescriptor, JobStatus, ModelKind, ModelDtype, ModelFormat, SlotType, WorkerStatus, InferenceCaps, CapabilitySource, GpuDevice) are referenced by name — the derive macro resolves them through the `anvilml-server` crate's dependency on `anvilml-core`. New `ToSchema` types from this task (HealthResponse, ComponentVersions, SubmitJobRequest, SubmitJobResponse, ListJobsParams, BulkClearParams, RemovedCount, ListModelsParams, ListArtifactsParams) are also listed.

### Step 5: Re-export ApiDoc from lib.rs

Add `pub mod openapi;` and `pub use openapi::ApiDoc;` to `crates/anvilml-server/src/lib.rs`.

### Step 6: Update anvilml-openapi main.rs

Replace the stub with real generation:

```rust
use anvilml_server::ApiDoc;
use std::fs;

fn main() {
    let openapi_json = ApiDoc::openapi()
        .to_pretty_json()
        .expect("failed to serialize OpenAPI spec");

    fs::write("api/openapi.json", &openapi_json)
        .expect("failed to write api/openapi.json");

    println!("Generated api/openapi.json ({} bytes)", openapi_json.len());
}
```

The `api/openapi.json` path is relative to the current working directory. The `anvilml-openapi` binary is invoked from the workspace root (`cargo run -p anvilml-openapi`), so the relative path resolves correctly.

### Step 7: Bump versions

Per ENVIRONMENT.md §12:
- `crates/anvilml-server/Cargo.toml`: version `0.1.26` → `0.1.27`
- `crates/anvilml-openapi/Cargo.toml`: bump patch version (read current from Cargo.toml, increment Z)

### Step 8: Verify idempotency

The plan produces a deterministic JSON output because:
- `utoipa::OpenApi::openapi()` generates a fixed schema from the compile-time derive macros.
- `to_pretty_json()` produces consistent formatting (sorted keys, consistent indentation).
- No runtime data (timestamps, random UUIDs, etc.) is included in the OpenAPI spec.

A second run of `cargo run -p anvilml-openapi` will produce byte-identical output.

## Public API Surface

New public items in `anvilml-server`:

| Item | Path | Description |
|------|------|-------------|
| `ApiDoc` | `anvilml_server::ApiDoc` | OpenAPI spec struct with derive(OpenApi) |
| `openapi` module | `anvilml_server::openapi` | Module containing ApiDoc |

The `#[utoipa::path]` annotations are compile-time macros — they do not introduce new runtime pub items. The `ToSchema` derives on response types are internal (`pub(crate)` structs), so they do not expand the public API surface.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/Cargo.toml` | Add utoipa dependency with features |
| Modify | `crates/anvilml-server/Cargo.toml` | Bump version 0.1.26 → 0.1.27 |
| Modify | `crates/anvilml-server/src/lib.rs` | Add `pub mod openapi;` and `pub use openapi::ApiDoc;` |
| CREATE | `crates/anvilml-server/src/openapi.rs` | ApiDoc struct with derive(OpenApi) |
| Modify | `crates/anvilml-server/src/handlers/health.rs` | Derive ToSchema on HealthResponse, add #[utoipa::path] on health() |
| Modify | `crates/anvilml-server/src/handlers/system.rs` | Derive ToSchema on ComponentVersions, add #[utoipa::path] on 3 handlers |
| Modify | `crates/anvilml-server/src/handlers/jobs.rs` | Derive ToSchema on request/response types, implement IntoParams on params, add #[utoipa::path] on 6 handlers |
| Modify | `crates/anvilml-server/src/handlers/models.rs` | Derive ToSchema on params, add #[utoipa::path] on 3 handlers |
| Modify | `crates/anvilml-server/src/handlers/workers.rs` | Add #[utoipa::path] on 2 handlers |
| Modify | `crates/anvilml-server/src/handlers/artifacts.rs` | Derive ToSchema on params, add #[utoipa::path] on 2 handlers |
| Modify | `crates/anvilml-server/src/handlers/nodes.rs` | Add #[utoipa::path] on 1 handler |
| Modify | `crates/anvilml-openapi/Cargo.toml` | Add utoipa dependency, bump version |
| Modify | `crates/anvilml-openapi/src/main.rs` | Replace stub with real OpenAPI generation |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| (none new) | N/A | This task does not introduce new test files. The acceptance criterion is the `cargo run -p anvilml-openapi` command producing valid JSON. The existing `anvilml-server` tests continue to pass because no handler logic changes. | `cargo run -p anvilml-openapi` exits 0 |
| (none new) | N/A | Idempotency — running the generator twice produces identical output. | `cargo run -p anvilml-openapi && cp api/openapi.json /tmp/run1.json && cargo run -p anvilml-openapi && diff /tmp/run1.json api/openapi.json && rm /tmp/run1.json` — exits 0 |
| (none new) | N/A | Every §13.4 route appears in the generated spec. | `python3 -c "import json; d=json.load(open('api/openapi.json')); paths=list(d['paths'].keys()); assert len(paths)>=17, f'expected >=17 routes, got {len(paths)}'; print('All routes present:', sorted(paths))"` — exits 0 |

## CI Impact

The `openapi-drift` CI job (defined in `.github/workflows/ci.yml`) runs `cargo run -p anvilml-openapi && git diff --exit-code api/openapi.json`. Currently this job checks against a stub/empty file. After this task, `api/openapi.json` will contain real content, making the gate meaningful: any future handler signature change that is not reflected in the annotations will cause the gate to fail. This task does NOT modify `.github/workflows/ci.yml` — that is the scope of P18-F2.

## Platform Considerations

None identified. The `utoipa` derive macros are compile-time only and produce platform-independent output. The file write uses `std::fs::write` which is cross-platform. The relative path `api/openapi.json` resolves correctly from the workspace root on all platforms.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `utoipa::OpenApi` derive macro may fail to resolve types across crate boundaries (e.g., `Job` from `anvilml-core` accessed through `anvilml-server`). | Medium | High | The `#[openapi(components(schemas(...)))]` attribute resolves types by name within the crate's scope. Since `anvilml-server` depends on `anvilml-core` and re-exports via `anvilml_core::Job`, the types must be referenced with their full path (e.g., `anvilml_core::Job`) or re-exported at the server crate level. If resolution fails, add `pub use anvilml_core::Job;` etc. to the openapi.rs module or use full paths in the schemas list. |
| `AnvilError` is not `ToSchema` — error responses will lack a schema object in the OpenAPI spec. | Medium | Low | Document error responses inline using `response = (status = 400, description = "Bad request — malformed JSON or invalid parameters")` without a `content` type. This is acceptable: the error body shape is documented in `ANVILML_DESIGN.md §13.5` and is consistent across all endpoints. |
| The `api/openapi.json` path is relative — if the binary is invoked from a different working directory, it fails silently or writes to the wrong location. | Low | Medium | The acceptance criterion runs `cargo run -p anvilml-openapi` from the workspace root (standard cargo behavior). Document this in the plan. If needed, use `std::env::current_dir()` to resolve the path dynamically, but this is unlikely to be an issue since the CI gate and all invocations happen from the repo root. |
| `utoipa` 5.5.0 `axum_extras` feature may not resolve axum `Json<T>` response types automatically. | Low | Medium | The `axum_extras` feature in utoipa 5.5.0 provides automatic response type inference for `axum::Json<T>`. If it fails for any type, fall back to explicit `response = (status = 200, description = "...", body = Some(MyType))` syntax. |

## Acceptance Criteria

- [ ] `cargo run -p anvilml-openapi` exits 0
- [ ] `python3 -c "import json, sys; d=json.load(open('api/openapi.json')); assert len(d['paths'])>=17, f'expected >=17 routes, got {len(d[\"paths\"])}'; print(f'Routes: {len(d[\"paths\"])}')"` exits 0
- [ ] `cargo run -p anvilml-openapi && cp api/openapi.json /tmp/run1.json && cargo run -p anvilml-openapi && diff /tmp/run1.json api/openapi.json && rm /tmp/run1.json` exits 0 (idempotency)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (no new warnings from added annotations)
- [ ] `cargo test --workspace --features mock-hardware` exits 0 (existing tests still pass)
