# Plan Report: P20-A1

| Field | Value |
|-------|-------|
| Task ID | P20-A1 |
| Phase | 020 — OpenAPI & Launcher Polish |
| Description | anvilml-server: utoipa annotations on all handlers + schemas |
| Depends on | P19-A3 |
| Project | anvilml |
| Planned at | 2026-06-12T05:00:00Z |
| Attempt | 1 |

## Objective

Add `#[utoipa::path(...)]` annotations to every REST handler in `anvilml-server` that is missing them, and ensure all request/response types used in those annotations derive `utoipa::ToSchema`. This produces a complete OpenAPI schema for the `anvilml-openapi` generator (P20-A2) without changing any handler behavior.

## Scope

### In Scope
- Add `utoipa::path` annotations to 8 handlers currently missing them:
  - `health::health` — GET /health
  - `system::get_env` — GET /v1/system/env
  - `system::get_system` — GET /v1/system
  - `models::list_models` — GET /v1/models
  - `models::get_model` — GET /v1/models/{id}
  - `models::rescan_models` — POST /v1/models/rescan
  - `workers::list_workers` — GET /v1/workers
  - `workers::restart_worker` — POST /v1/workers/{id}/restart
- Add `utoipa::ToSchema` derive to 2 response types:
  - `HealthResponse` in `handlers/health.rs`
  - `RescanResponse` in `handlers/models.rs`
- Add `use utoipa::ToSchema` imports where needed
- Add `use utoipa::ToSchema` import in `handlers/models.rs` for `RescanResponse`
- Verify all 6 handlers already annotated in `jobs.rs` and `artifacts.rs` have correct parameters/responses

### Out of Scope
- Adding `GET /v1/system/versions` handler (deferred to later phase; endpoint not yet implemented)
- Adding `ProvisioningState` field to `EnvReport` (deferred to later phase; type not yet in codebase)
- Any behavioral changes to handler implementations
- Modifying `anvilml-openapi` crate (that is P20-A2)
- Modifying CI workflow files (that is P20-A4)
- Browser auto-open (that is P20-A3)

## Approach

### Step 1: Add `ToSchema` to `HealthResponse`
**File:** `crates/anvilml-server/src/handlers/health.rs`
- Add `use utoipa::ToSchema;` import
- Add `ToSchema` to the derive list on `HealthResponse` struct

### Step 2: Add `ToSchema` to `RescanResponse`
**File:** `crates/anvilml-server/src/handlers/models.rs`
- Add `use utoipa::ToSchema;` import
- Add `ToSchema` to the derive list on `RescanResponse` struct

### Step 3: Annotate `health::health` handler
**File:** `crates/anvilml-server/src/handlers/health.rs`
- Add `#[utoipa::path(get, path = "/health", summary = "Health check", responses((status = 200, description = "Service is healthy", body = HealthResponse)))]`
- Place above the existing doc comment

### Step 4: Annotate `system::get_env` handler
**File:** `crates/anvilml-server/src/handlers/system.rs`
- Add `use utoipa::ToSchema;` import (already present via re-export)
- Add `#[utoipa::path(get, path = "/v1/system/env", summary = "Get Python environment health report", responses((status = 200, description = "Environment report", body = EnvReport)))]`

### Step 5: Annotate `system::get_system` handler
**File:** `crates/anvilml-server/src/handlers/system.rs`
- Add `#[utoipa::path(get, path = "/v1/system", summary = "Get hardware information", responses((status = 200, description = "Hardware info", body = HardwareInfo)))]`

### Step 6: Annotate `models::list_models` handler
**File:** `crates/anvilml-server/src/handlers/models.rs`
- Add `use utoipa::ToSchema;` import
- Add `#[utoipa::path(get, path = "/v1/models", summary = "List scanned models", params(("kind" = Option<ModelKind>, Query, description = "Filter by model kind")), responses((status = 200, description = "Model list", body = Vec<ModelMeta>)))]`

### Step 7: Annotate `models::get_model` handler
**File:** `crates/anvilml-server/src/handlers/models.rs`
- Add `#[utoipa::path(get, path = "/v1/models/{id}", summary = "Get a model by ID", params(("id" = String, Path, description = "Model ID")), responses((status = 200, description = "Model found", body = ModelMeta), (status = 404, description = "Model not found")))]`

### Step 8: Annotate `models::rescan_models` handler
**File:** `crates/anvilml-server/src/handlers/models.rs`
- Add `#[utoipa::path(post, path = "/v1/models/rescan", summary = "Trigger model directory rescan", responses((status = 202, description = "Rescan started", body = RescanResponse)))]`

### Step 9: Annotate `workers::list_workers` handler
**File:** `crates/anvilml-server/src/handlers/workers.rs`
- Add `use utoipa::ToSchema;` import
- Add `#[utoipa::path(get, path = "/v1/workers", summary = "List worker pool status", responses((status = 200, description = "Worker list", body = Vec<WorkerInfo>)))]`

### Step 10: Annotate `workers::restart_worker` handler
**File:** `crates/anvilml-server/src/handlers/workers.rs`
- Add `use utoipa::ToSchema;` import
- Add `#[utoipa::path(post, path = "/v1/workers/{id}/restart", summary = "Restart a worker", params(("id" = String, Path, description = "Worker ID")), responses((status = 202, description = "Worker restarting"), (status = 404, description = "Worker not found"), (status = 500, description = "Restart failed")))]`

### Step 11: Verify existing annotations
- Review the 6 already-annotated handlers in `jobs.rs` and `artifacts.rs` to confirm method/path/responses are correct
- No changes expected; this is a verification step

### Step 12: Build and lint
- Run `cargo build -p anvilml-server --features mock-hardware` — must exit 0
- Run `cargo clippy -p anvilml-server --features mock-hardware -- -D warnings` — must exit 0

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/handlers/health.rs` | Add `ToSchema` derive to `HealthResponse`; add `utoipa::path` annotation to `health()` |
| Modify | `crates/anvilml-server/src/handlers/system.rs` | Add `utoipa::path` annotations to `get_env()` and `get_system()` |
| Modify | `crates/anvilml-server/src/handlers/models.rs` | Add `ToSchema` derive to `RescanResponse`; add `use utoipa::ToSchema`; add `utoipa::path` annotations to `list_models()`, `get_model()`, `rescan_models()` |
| Modify | `crates/anvilml-server/src/handlers/workers.rs` | Add `use utoipa::ToSchema`; add `utoipa::path` annotations to `list_workers()` and `restart_worker()` |

## Tests

None. This task adds only annotations and derives — no behavioral changes, no new code paths, no new public API. The existing test suite (`cargo test -p anvilml-server --features mock-hardware`) validates that handlers still function correctly.

## CI Impact

No CI changes. The `anvilml-openapi` generator (P20-A2) will consume these annotations to produce `backend/openapi.json`, and the OpenAPI diff gate (P20-A4) will verify it. This task alone does not modify any CI file.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `utoipa::path` annotation conflicts with existing doc comments | Low | Medium | Place annotation before doc comment; use `#[doc(hidden)]` on annotation if needed |
| Missing `ToSchema` on a type referenced in a response causes compile error | Low | Medium | All types are pre-verified: `HealthResponse` and `RescanResponse` will get `ToSchema`; all core types already derive it |
| Query parameter type mismatch (e.g. `Option<ModelKind>` not serializable) | Low | Low | `ModelKind` derives `Serialize` and `ToSchema`; `Option<T>` is supported by utoipa |
| `serde_json::Value` in responses not appearing in OpenAPI schema | Low | Low | `serde_json::Value` maps to `object` type in utoipa; existing annotations already use this pattern |

## Acceptance Criteria

- [ ] `HealthResponse` derives `utoipa::ToSchema`
- [ ] `RescanResponse` derives `utoipa::ToSchema`
- [ ] All 14 REST handlers have `#[utoipa::path(...)]` annotations (6 existing + 8 new)
- [ ] `cargo build -p anvilml-server --features mock-hardware` exits 0
- [ ] `cargo clippy -p anvilml-server --features mock-hardware -- -D warnings` exits 0
- [ ] No behavioral changes to any handler (verified by existing test suite)
