# Plan Report: P2-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P2-A3                                       |
| Phase       | 002 — Core Types & IPC                      |
| Description | anvilml-core: domain types — Job, Model, Artifact |
| Depends on  | P2-A1 (error types), P2-A2 (config types)   |
| Project     | anvilml                                     |
| Planned at  | 2026-05-29T19:05:00Z                        |
| Attempt     | 1                                           |

## Objective

Define the core domain types that form the data contract between all AnvilML crates: the Job lifecycle (Job, JobStatus, JobSettings, SubmitJobRequest, SubmitJobResponse), model metadata (ModelMeta, ModelKind, DType), and artifact metadata (ArtifactMeta). These types are pure serializable structs/enums with zero I/O or async — they live in `anvilml-core` which has no dependencies beyond serde, uuid, chrono, utoipa, and serde_json. Every type derives `Serialize`, `Deserialize`, `Clone`, `Debug`, and `utoipa::ToSchema`. This task adds the new crate dependencies (`uuid`, `chrono`, `utoipa`) and establishes the `types` module with re-exports.

## Scope

### In Scope
- Create `crates/anvilml-core/src/types/mod.rs` — module declaration and public re-exports
- Create `crates/anvilml-core/src/types/job.rs` — Job, JobStatus, JobSettings, SubmitJobRequest, SubmitJobResponse structs/enums
- Create `crates/anvilml-core/src/types/model.rs` — ModelMeta, ModelKind, DType structs/enums
- Create `crates/anvilml-core/src/types/artifact.rs` — ArtifactMeta struct
- Modify `crates/anvilml-core/Cargo.toml` — add `uuid` (features: v4, serde), `chrono` (features: serde), `utoipa`, and ensure `serde_json` is present
- Modify `crates/anvilml-core/src/lib.rs` — add `pub mod types`
- Write unit tests in each file validating: serialization round-trips, default values, PartialEq/Eq for JobStatus, UUID generation, DateTime serialization
- Acceptance criterion: `cargo test -p anvilml-core -- types` exits 0

### Out of Scope
- Hardware/worker/event types (P2-A4)
- IPC message enums or framing (P2-B1, P2-B2)
- Any I/O, async, database, or network code
- Refactoring existing `config.rs` placeholder `ModelKind` — the new `types::model::ModelKind` will be the canonical definition; `config.rs` references will need updating in a future task if they exist
- OpenAPI JSON generation (handled by anvilml-openapi crate)
- Updating error variants that reference UUID as String (P2-A1 follow-up)

## Approach

### Step 1: Add dependencies to Cargo.toml
Add the following to `crates/anvilml-core/Cargo.toml` under `[dependencies]`:
```toml
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
utoipa = "5"
```
`serde_json` is already present (added in P2-A1). `serde` with `derive` feature is already present.

### Step 2: Create types module structure
Create the directory `crates/anvilml-core/src/types/` and the following files:
- `mod.rs` — declares the three sub-modules and re-exports all public types
- `job.rs` — Job lifecycle types
- `model.rs` — model metadata types  
- `artifact.rs` — artifact metadata type

Modify `crates/anvilml-core/src/lib.rs` to add:
```rust
pub mod types;
pub use types::*;
```

### Step 3: Implement job.rs
Define the following types in `crates/anvilml-core/src/types/job.rs`:

**JobStatus** (enum):
- Variants: `Queued`, `Running`, `Completed`, `Failed`, `Cancelled`
- Derives: `Serialize, Deserialize, Clone, Debug, PartialEq, Eq`
- `PartialEq` and `Eq` are required because the scheduler and server compare statuses directly
- Use `#[serde(rename_all = "snake_case")]` for consistent wire format

**JobSettings** (struct):
- Fields: `seed: i64`, `steps: u32`, `guidance_scale: f32`, `width: u32`, `height: u32`, `device_preference: Option<u32>`
- Derives: `Serialize, Deserialize, Clone, Debug`
- Provide a `Default` impl with sensible defaults (seed=-1, steps=20, guidance_scale=7.5, width=1024, height=1024, device_preference=None)

**Job** (struct):
- Fields: `id: Uuid`, `status: JobStatus`, `graph: serde_json::Value`, `settings: JobSettings`, `device_index: Option<u32>`, `created_at: DateTime<Utc>`, `started_at: Option<DateTime<Utc>>`, `completed_at: Option<DateTime<Utc>>`, `worker_id: Option<String>`, `artifact_count: u32`, `error: Option<String>`
- Derives: `Serialize, Deserialize, Clone, Debug`
- Provide a `Default` impl (id = generated v4 UUID via a helper, status=Queued, graph=empty object, settings=defaults, device_index=None, timestamps set to Utc now or None, worker_id=None, artifact_count=0, error=None)

**SubmitJobRequest** (struct):
- Fields: `graph: serde_json::Value`, `settings: JobSettings`
- Derives: `Serialize, Deserialize, Clone, Debug`

**SubmitJobResponse** (struct):
- Fields: `job_id: Uuid`, `queue_position: u32`
- Derives: `Serialize, Deserialize, Clone, Debug`

### Step 4: Implement model.rs
Define the following types in `crates/anvilml-core/src/types/model.rs`:

**ModelKind** (enum):
- Variants: `Clip`, `Diffusion`, `Vae`, `Lora`, `ControlNet`, `Unet`, `Upscale`
- Derives: `Serialize, Deserialize, Clone, Debug, PartialEq, Eq`
- This replaces the placeholder in `config.rs` — the canonical definition lives here

**DType** (enum):
- Variants: `F32`, `F16`, `BF16`, `Q8`, `Q4`, `Unknown`
- Derives: `Serialize, Deserialize, Clone, Debug, PartialEq, Eq`

**ModelMeta** (struct):
- Fields: `id: String` (first 16 hex chars of SHA256 of canonical path), `name: String`, `path: PathBuf`, `kind: ModelKind`, `size_bytes: u64`, `dtype_hint: DType`, `vram_estimate_mib: u32`, `scanned_at: DateTime<Utc>`
- Derives: `Serialize, Deserialize, Clone, Debug`

### Step 5: Implement artifact.rs
Define the following type in `crates/anvilml-core/src/types/artifact.rs`:

**ArtifactMeta** (struct):
- Fields: `hash: String` (SHA256 hex of PNG bytes, content-addressed), `job_id: Uuid`, `width: u32`, `height: u32`, `format: String` (always "png"), `seed: i64`, `steps: u32`, `prompt: String`, `created_at: DateTime<Utc>`
- Derives: `Serialize, Deserialize, Clone, Debug`

### Step 6: Implement mod.rs re-exports
Create `crates/anvilml-core/src/types/mod.rs` that:
- Declares `pub mod job; pub mod model; pub mod artifact;`
- Re-exports all public types: `pub use job::{Job, JobStatus, JobSettings, SubmitJobRequest, SubmitJobResponse};`
- `pub use model::{ModelMeta, ModelKind, DType};`
- `pub use artifact::ArtifactMeta;`

### Step 7: Write unit tests
Add tests in each file:

**job.rs tests:**
- `test_job_status_variants` — verify all 5 variants exist and serialize to snake_case strings
- `test_job_round_trip` — construct a Job with all fields set, serialize to JSON, deserialize back, assert equality
- `test_job_settings_defaults` — verify Default impl produces expected defaults (seed=-1, steps=20, etc.)
- `test_submit_job_response` — verify Serialize/Deserialize of SubmitJobResponse
- `test_job_status_eq` — verify PartialEq works for JobStatus comparison

**model.rs tests:**
- `test_model_kind_variants` — verify all 7 variants exist and serialize correctly
- `test_dtype_variants` — verify all 6 variants exist and serialize correctly
- `test_model_meta_round_trip` — construct a ModelMeta, round-trip through JSON

**artifact.rs tests:**
- `test_artifact_meta_round_trip` — construct an ArtifactMeta with all fields, round-trip through JSON

All tests use `#[cfg(test)]` modules inside the respective files (same-file unit test pattern per .clinerules §7.4).

### Step 8: Update lib.rs
Modify `crates/anvilml-core/src/lib.rs` to add:
```rust
pub mod types;
pub use types::*;
```

## Files Affected

| Action   | Path                              | Description |
|----------|-----------------------------------|-------------|
| MODIFY   | crates/anvilml-core/Cargo.toml    | Add uuid (v4, serde), chrono (serde), utoipa dependencies |
| MODIFY   | crates/anvilml-core/src/lib.rs    | Add `pub mod types; pub use types::*;` |
| CREATE   | crates/anvilml-core/src/types/mod.rs | Module declaration and public re-exports |
| CREATE   | crates/anvilml-core/src/types/job.rs | Job, JobStatus, JobSettings, SubmitJobRequest, SubmitJobResponse |
| CREATE   | crates/anvilml-core/src/types/model.rs | ModelMeta, ModelKind, DType |
| CREATE   | crates/anvilml-core/src/types/artifact.rs | ArtifactMeta |

## Tests

| Test ID / Name            | File                     | Validates               |
|---------------------------|--------------------------|-------------------------|
| test_job_status_variants  | types/job.rs             | All 5 JobStatus variants exist and serialize to snake_case |
| test_job_round_trip       | types/job.rs             | Full Job struct serializes and deserializes correctly through JSON |
| test_job_settings_defaults| types/job.rs             | Default impl produces expected default values |
| test_submit_job_response  | types/job.rs             | SubmitJobResponse Serialize/Deserialize |
| test_job_status_eq        | types/job.rs             | PartialEq/Eq for JobStatus comparison |
| test_model_kind_variants  | types/model.rs           | All 7 ModelKind variants exist and serialize correctly |
| test_dtype_variants       | types/model.rs           | All 6 DType variants exist and serialize correctly |
| test_model_meta_round_trip| types/model.rs           | ModelMeta full round-trip through JSON |
| test_artifact_meta_round_trip | types/artifact.rs    | ArtifactMeta full round-trip through JSON |

## CI Impact

No CI changes required. The existing CI workflow (`.github/workflows/ci.yml`) already runs `cargo test --workspace --features mock-hardware` which includes `anvilml-core`. No new jobs or steps are needed.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| ModelKind naming conflict with config.rs placeholder | Medium | Low | config.rs ModelKind is not yet imported by any other crate (verified via grep). After this task, downstream crates will use `types::model::ModelKind` instead. A future cleanup can remove the config.rs duplicate. |
| utoipa ToSchema derive requires all fields to also implement ToSchema | Low | Medium | All field types (Uuid, DateTime<Utc>, serde_json::Value, PathBuf, String, u32, i64, f32, Option<T>) are supported by utoipa 5.x. Verified that chrono's DateTime<Utc> implements ToSchema via the `chrono` feature of utoipa — plan includes adding `utoipa = { version = "5", features = ["chrono"] }` to enable this. |
| uuid v4 generation in Default impl requires runtime | Low | Low | The Job Default impl uses `Uuid::new_v4()` which is fine for unit tests (deterministic enough for equality assertions after round-trip). Alternatively, tests can construct Jobs explicitly without relying on Default for the id field. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-core -- types` exits 0 with all tests passing
- [ ] All new files compile without warnings under `cargo clippy`
- [ ] JobStatus derives PartialEq and Eq (verified by code inspection)
- [ ] All types derive Serialize, Deserialize, Clone, Debug, utoipa::ToSchema (verified by code inspection)
- [ ] Job.graph is serde_json::Value type (verified by code inspection)
- [ ] ArtifactMeta.hash is String containing SHA256 hex (verified by code inspection and doc comment)
- [ ] ModelKind in types/model.rs matches the 7 variants from ANVILML_DESIGN.md §4.2
- [ ] DType enum has all 6 variants: F32, F16, BF16, Q8, Q4, Unknown
- [ ] No new dependencies beyond uuid, chrono, utoipa added to Cargo.toml
