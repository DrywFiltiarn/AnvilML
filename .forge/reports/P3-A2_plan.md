# Plan Report: P3-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P3-A2                                       |
| Phase       | 003 — Core Domain Types                     |
| Description | anvilml-core: Job domain types              |
| Depends on  | P3-A1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-01T09:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Add the Job domain types to `anvilml-core` as specified in ANVILML_DESIGN §4.1.
This introduces the `Job` struct, `JobStatus` enum, `JobSettings` struct,
`SubmitJobRequest`, and `SubmitJobResponse` — all serializable, clonable, debuggable,
and schema-annotated for OpenAPI generation. New dependencies (`chrono`, `utoipa`,
`serde_json`) are added to `anvilml-core/Cargo.toml`. Unit tests verify
round-trip serialization and field correctness.

## Scope

### In Scope
- Add `chrono` (0.4, serde feature), `utoipa` (5, serde_json + chrono features),
  and `serde_json` (1) to `anvilml-core/Cargo.toml`
- Create `crates/anvilml-core/src/types/mod.rs` module file
- Create `crates/anvilml-core/src/types/job.rs` with all five types:
  - `JobStatus` enum (Queued, Running, Completed, Failed, Cancelled)
  - `JobSettings` struct (seed, steps, guidance_scale, width, height,
    device_preference)
  - `Job` struct (id, status, graph, settings, device_index, created_at,
    started_at, completed_at, worker_id, artifact_count, error)
  - `SubmitJobRequest` struct (graph, settings)
  - `SubmitJobResponse` struct (job_id, queue_position)
- All types derive `Serialize`, `Deserialize`, `Clone`, `Debug`,
  and `utoipa::ToSchema`; `JobStatus` additionally derives `PartialEq`, `Eq`
- `Job.graph` uses `serde_json::Value`
- Timestamps use `chrono::DateTime<Utc>` with ISO 8601 serialization
- Update `lib.rs` to export the new `types` module
- Unit tests in `src/types/job.rs` under `mod tests` verifying:
  - JSON round-trip serialization for each type
  - `JobStatus` variant count and equality
  - `JobSettings` field defaults via serde defaults
  - `DateTime<Utc>` serializes as ISO 8601 string
  - `serde_json::Value` round-trips for graph field
- `cargo test -p anvilml-core -- job` exits 0

### Out of Scope
- Model/Artifact types (P3-A3)
- Hardware/Worker types (P3-A4)
- WebSocket event types (P3-A5)
- HTTP handler implementation (P3-A6)
- Any I/O or async logic in core
- Changes to `anvilml-server` or other crates
- CI workflow modifications
- `#[serde(default)]` for all fields (only where specified: optional timestamps,
  optional string fields)

## Approach

1. **Update Cargo.toml** — Add three new dependencies to
   `crates/anvilml-core/Cargo.toml`:
   - `chrono = { version = "0.4", features = ["serde"] }`
     for `DateTime<Utc>` timestamps with ISO 8601 serialization.
   - `utoipa = { version = "5", features = ["serde_json", "chrono"] }`
     for `ToSchema` derive support of serde_json and chrono types.
   - `serde_json = "1"` for the `serde_json::Value` type used in
     `Job.graph`.

2. **Create `src/types/mod.rs`** — Declare the two sub-modules:
   ```rust
   pub mod job;
   ```

3. **Create `src/types/job.rs`** — Define all five types per ANVILML_DESIGN §4.1:
   - Import `Uuid`, `DateTime<Utc>`, `serde_json::Value` and derive macros.
   - Derive `Serialize, Deserialize, Clone, Debug, ToSchema` on all structs.
   - Derive `Serialize, Deserialize, Clone, Debug, PartialEq, Eq, ToSchema`
     on `JobStatus` enum.
   - Add `#[serde(default)]` on optional fields so deserialization from
     partial JSON (e.g. from DB) does not fail.
   - Write unit tests:
     a. `job_status_variants` — assert `JobStatus` has exactly 5 variants;
        verify all pairs compare equal/unequal correctly.
     b. `job_settings_roundtrip` — serialize/deserialize `JobSettings`
        and assert fields round-trip.
     c. `job_roundtrip` — construct a `Job` with all fields populated,
        serialize to JSON, deserialize back, assert every field matches.
     d. `job_graph_json_value` — verify that a `serde_json::Value` graph
        (arbitrary object) round-trips through `Job` serialization.
     e. `submit_job_request_roundtrip` — serialize/deserialize
        `SubmitJobRequest` and assert graph + settings preserved.
     f. `submit_job_response_roundtrip` — serialize/deserialize
        `SubmitJobResponse` and assert job_id + queue_position preserved.
     g. `job_timestamps_iso8601` — verify that `DateTime<Utc>` fields
        serialize to valid ISO 8601 strings (e.g. contains 'T' and 'Z').

4. **Update `src/lib.rs`** — Add `pub mod types;` and re-export the types
   module contents so downstream crates can access them via
   `anvilml_core::types::*`.

5. **Verify** — Run `cargo test -p anvilml-core -- job` to confirm all
   tests pass with exit code 0.

## Files Affected

| Action   | Path                                              | Description                                      |
|----------|---------------------------------------------------|--------------------------------------------------|
| MODIFY   | crates/anvilml-core/Cargo.toml                    | Add chrono, utoipa, serde_json dependencies      |
| CREATE   | crates/anvilml-core/src/types/mod.rs              | Module declaration for job types                 |
| CREATE   | crates/anvilml-core/src/types/job.rs              | Job, JobStatus, JobSettings, SubmitJobRequest,   |
|          |                                                   | SubmitJobResponse + unit tests                   |
| MODIFY   | crates/anvilml-core/src/lib.rs                    | Add `pub mod types;` and re-export               |

## Tests

| Test ID / Name              | File                                  | Validates                                       |
|-----------------------------|---------------------------------------|-------------------------------------------------|
| job_status_variants         | src/types/job.rs                      | JobStatus has 5 variants; equality/inequality   |
| job_settings_roundtrip      | src/types/job.rs                      | JobSettings serializes and deserializes correctly |
| job_roundtrip               | src/types/job.rs                      | Full Job struct round-trips with all fields     |
| job_graph_json_value        | src/types/job.rs                      | serde_json::Value graph field preserves content |
| submit_job_request_roundtrip| src/types/job.rs                      | SubmitJobRequest serializes/deserializes        |
| submit_job_response_roundtrip| src/types/job.rs                     | SubmitJobResponse serializes/deserializes       |
| job_timestamps_iso8601      | src/types/job.rs                      | DateTime<Utc> fields produce ISO 8601 strings   |

## CI Impact

No CI changes required. The task only adds dependencies and source files to
`anvilml-core`. Existing CI jobs (`rust`, `rust-windows`) already run
`cargo test -p anvilml-core` and `cargo clippy -p anvilml-core`; the new
types are covered by those gates. No new workflow jobs or steps are needed.

## Risks and Mitigations

| Risk                                      | Likelihood | Impact | Mitigation                                          |
|-------------------------------------------|-----------|--------|-----------------------------------------------------|
| utoipa version incompatibility with serde_json/chrono features | Low       | Medium | Verify feature flags against docs.rs before writing; use well-known stable versions (utoipa 5) |
| `DateTime<Utc>` serialization format mismatch between chrono and utoipa | Low       | Low    | Use `#[serde(with = "chrono::serde::ts_seconds")]` or default ISO 8601 format; test explicitly |
| `serde_json::Value` ToSchema annotation missing in utoipa   | Medium      | Medium | Check utoipa docs for Value support; if unavailable, use `#[schema(value_type = Object)]` attribute |
| Circular dependency: anvilml-core adding serde_json which other crates may not expect | Low       | Low    | serde_json is a leaf-level dep; no crate depends on core for JSON logic |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-core -- job` exits 0 with all 7 tests passing
- [ ] `crates/anvilml-core/Cargo.toml` contains dependencies: chrono (serde),
      utoipa (serde_json, chrono features), serde_json
- [ ] `crates/anvilml-core/src/types/job.rs` defines JobStatus enum with exactly
      5 variants: Queued, Running, Completed, Failed, Cancelled
- [ ] All five types (Job, JobStatus, JobSettings, SubmitJobRequest,
      SubmitJobResponse) derive Serialize, Deserialize, Clone, Debug, ToSchema
- [ ] JobStatus additionally derives PartialEq, Eq
- [ ] `Job.graph` is typed as `serde_json::Value`
- [ ] Timestamp fields use `DateTime<Utc>` and serialize as ISO 8601 strings
- [ ] `crates/anvilml-core/src/lib.rs` exports `pub mod types;`
