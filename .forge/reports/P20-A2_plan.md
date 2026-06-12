# Plan Report: P20-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P20-A2                                      |
| Phase       | 020 — OpenAPI & Launcher Polish             |
| Description | anvilml-openapi: generate backend/openapi.json |
| Depends on  | P20-A1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-06-12T06:10:00Z                        |
| Attempt     | 1                                           |

## Objective

Implement `crates/anvilml-openapi/src/main.rs` so that it uses `utoipa`'s `#[derive(OpenApi)]` macro to produce a complete OpenAPI 3.1 specification from all `anvilml-server` handler annotations and component schemas, serialises the result to pretty-printed JSON, and writes it to `backend/openapi.json` in the repository root.

## Scope

### In Scope
- Implement `anvilml-openapi/src/main.rs`:
  - Define an `OpenApi` derive struct referencing every handler path group (health, system, jobs, models, workers, artifacts, WS events)
  - Register all component schemas referenced by those handlers (request/response bodies, error types, WsEvent variants)
  - Serialize the spec to pretty JSON and write `backend/openapi.json`
- Add `utoipa` and `serde_json` dependencies to `anvilml-openapi/Cargo.toml`
- Verify: `cargo run -p anvilml-openapi` produces a non-empty `backend/openapi.json` containing all `/v1` paths and the error response shape

### Out of Scope
- Modifying any handler file, schema type, or server code (P20-A1 owns that)
- Browser auto-open (P20-A3)
- CI workflow changes (P20-A4)
- Manual editing of `backend/openapi.json` — it is generated and committed

## Approach

1. **Update `anvilml-openapi/Cargo.toml`** — add `utoipa` (with `json` feature for serialization) and `serde_json` as dependencies. Both are available via workspace: `utoipa = { workspace = true }` with `features = ["json"]`, `serde_json = { workspace = true }`.

2. **Implement `anvilml-openapi/src/main.rs`**:
   a. Import the handler modules from `anvilml_server::handlers` (health, system, jobs, models, workers, artifacts) and the `WsEvent` type from `anvilml_core::types::events`.
   b. Define a single `#[derive(OpenApi)]` struct named `OpenApiSpec` with:
      - `paths` attribute referencing all handler functions via module paths (e.g., `anvilml_server::handlers::health::health`, `anvilml_server::handlers::jobs::submit_job`, etc.)
      - `components` attribute registering all schema types: `HealthResponse`, `EnvReport`, `HardwareInfo`, `SubmitJobRequest`, `SubmitJobResponse`, `ErrorInline`, `ClearJobsResponse`, `RescanResponse`, `ArtifactMeta`, `Job`, `ModelMeta`, `WorkerInfo`, `WsEvent` and all its variant structs (`SystemStatsEvent`, `JobQueuedEvent`, `JobStartedEvent`, `JobProgressEvent`, `JobImageReadyEvent`, `JobCompletedEvent`, `JobFailedEvent`, `JobCancelledEvent`, `WorkerStatusChangedEvent`), `GpuStatSnapshot`, `JobStatus`, `ModelKind`, `DeviceType`, `WorkerStatus`, `EnumerationSource`, `CapabilitySource`, `InferenceCaps`, `GpuDevice`, `HardwareInfo`, `HostInfo`, `EnvReport`, `AnvilError`
      - `info` attribute with title, version
   c. In `main()`: call `OpenApiSpec::openapi()` to get the `OpenApi` struct, serialize via `serde_json::to_string_pretty`, write to `../../backend/openapi.json` (relative to `crates/anvilml-openapi/src/main.rs`, the output path is `backend/openapi.json` from workspace root).

3. **Write the output file**: Use `std::fs::write` with the pretty-printed JSON string. The output path is computed relative to the workspace root: read `std::env::var("CARGO_MANIFEST_DIR")` and traverse up two levels to the workspace root, then append `backend/openapi.json`.

4. **Verification plan** (for ACT session): Run `cargo run -p anvilml-openapi`, confirm `backend/openapi.json` is non-empty, contains all `/v1/*` paths, and includes the error response shape.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-openapi/Cargo.toml` | Add `utoipa` (with `json` feature) and `serde_json` dependencies |
| Modify | `crates/anvilml-openapi/src/main.rs` | Implement OpenAPI spec generation (replace stub) |
| Create | `backend/openapi.json` | Generated OpenAPI 3.1 spec output |

## Tests

None. The `anvilml-openapi` crate is a build-time binary that generates a file. No unit or integration tests are needed — correctness is verified by running the binary and inspecting the output, which is covered by the acceptance criteria and the CI openapi-diff gate (P20-A4).

## CI Impact

No direct CI changes. The CI openapi-diff gate (`cargo run -p anvilml-openapi && git diff --exit-code backend/openapi.json`) is implemented in P20-A4. This task produces the `backend/openapi.json` file that the gate will verify. Once P20-A2 is complete, the gate will fail until P20-A4 adds it to the CI workflow.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `utoipa::OpenApi` derive macro cannot reference handler paths from a sibling crate due to module visibility | Medium | High | Use `anvilml_server::handlers::*` paths directly; ensure handler modules are `pub mod` (they are). If the derive macro cannot resolve cross-crate paths, use `path = "module::path"` string syntax instead. |
| Schema types from `anvilml-core` are not exported publicly for the derive macro to reference | Low | Medium | All types already derive `ToSchema` and are `pub`. If some are re-exported through `anvilml_core`, use the re-export path. |
| `backend/openapi.json` path resolution fails at runtime | Low | Low | Use `CARGO_MANIFEST_DIR` env var to compute workspace root reliably; fall back to `"../backend/openapi.json"` relative to manifest dir. |
| Missing handler paths in the OpenApi struct produce incomplete spec | Medium | High | Systematically enumerate all 16 handler functions from `lib.rs` routes and all 7 handler files; cross-check against the route table in `ARCHITECTURE.md §7`. |
| `WsEvent` variants need explicit schema registration | Low | Medium | All variant structs already derive `ToSchema`; register them individually in the `components` section. |

## Acceptance Criteria

- [ ] `cargo run -p anvilml-openapi` exits 0 and writes a non-empty `backend/openapi.json`
- [ ] `backend/openapi.json` contains all `/v1` paths: `/v1/system`, `/v1/system/env`, `/v1/jobs`, `/v1/jobs/{id}`, `/v1/jobs/{id}/cancel`, `/v1/models`, `/v1/models/{id}`, `/v1/models/rescan`, `/v1/workers`, `/v1/workers/{id}/restart`, `/v1/artifacts`, `/v1/artifacts/{hash}`, and `/v1/events` (WS)
- [ ] `backend/openapi.json` contains `/health` endpoint
- [ ] `backend/openapi.json` includes the error response shape (`ErrorInline` with `error`, `message`, `request_id` fields) in the components/schemas section
- [ ] `backend/openapi.json` includes `WsEvent` and all 9 variant structs as component schemas
- [ ] The generated JSON is valid and pretty-printed (indented with spaces)
