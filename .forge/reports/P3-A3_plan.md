# Plan Report: P3-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P3-A3                                       |
| Phase       | 003 — Core Domain Types: Data Model         |
| Description | anvilml-core: ArtifactMeta type             |
| Depends on  | P3-A2                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-28T15:58:00Z                        |
| Attempt     | 1                                           |

## Objective

Define the `ArtifactMeta` struct in `crates/anvilml-core/src/types/artifact.rs` as a pure data type representing metadata for a generated, content-addressed PNG artifact. This type carries the SHA-256 content hash, the originating job ID, generation parameters (width, height, seed, steps), creation timestamp, and file path. It introduces the `utoipa` dependency (v5.5.0) to provide the `ToSchema` derive macro for future OpenAPI annotation. The deliverable is a compile-ready type with full serde roundtrip support and ≥3 tests.

## Scope

### In Scope
- Create `crates/anvilml-core/src/types/artifact.rs` with the `ArtifactMeta` struct containing fields: `hash` (String), `job_id` (Uuid), `width` (u32), `height` (u32), `seed` (i64), `steps` (u32), `created_at` (DateTime<Utc>), `file_path` (PathBuf).
- Derive `Debug`, `Clone`, `Serialize`, `Deserialize`, `ToSchema` on `ArtifactMeta`.
- Add `utoipa` dependency to `crates/anvilml-core/Cargo.toml` with features `uuid` and `chrono` for the derive macro's OpenAPI schema generation.
- Add `mod artifact;` and `pub use artifact::*;` to `crates/anvilml-core/src/types/mod.rs`.
- Create `crates/anvilml-core/tests/artifact_tests.rs` with ≥3 tests covering construction and serde roundtrip.

### Out of Scope
defers_to (from JSON): [] — absent. This task may not defer any scope. No out-of-scope bullets naming deferred functionality are permitted.

## Existing Codebase Assessment

The `anvilml-core` crate already has a well-established pattern for domain types in `src/types/`. Two type modules exist: `job.rs` (Job, JobStatus, JobSettings) and `model.rs` (ModelMeta, ModelKind, ModelDtype, ModelFormat). Both follow the same conventions:

- **Imports**: `chrono::{DateTime, Utc}`, `serde::{Deserialize, Serialize}`, `uuid::Uuid`, and `std::path::PathBuf`.
- **Derives**: `Debug, Clone, Serialize, Deserialize` on all structs; `PartialEq, Eq` on structs that are compared in tests.
- **Doc comments**: Every struct has a one-sentence `///` doc comment describing what it represents. Every field has an inline `///` comment explaining its purpose.
- **Test style**: Integration tests in `tests/` directory that construct types with all fields populated, serialise to JSON via `serde_json::to_string`, deserialise back, and assert equality. Additional tests verify individual enum variant serialization and null-field roundtrips.
- **Module declaration**: `types/mod.rs` declares each submodule with `pub mod <name>;` and re-exports with `pub use <name>::*;`.

`anvilml-core/Cargo.toml` currently has dependencies: `thiserror`, `axum`, `uuid` (with `v4` and `serde` features), `serde_json`, `serde` (with `derive` feature), `sqlx` (with `sqlite` feature), `toml`, and `chrono` (with `serde` feature). Dev dependencies include `tokio` and `serial_test`.

No prior source exists for `artifact.rs` — this task creates it from scratch. The design doc (ANVILML_DESIGN.md §5.1) confirms `artifact.rs` is the expected file in the module layout.

## Resolved Dependencies

| Type   | Name   | Version verified | MCP source     | Feature flags confirmed          |
|--------|--------|-----------------|----------------|----------------------------------|
| crate  | utoipa | 5.5.0           | rust-docs MCP  | uuid, chrono (macros is default) |

The `utoipa` crate version 5.5.0 is the latest stable release (confirmed via `rust-docs_get_crate_info` and `rust-docs_get_crate_versions`). The `macros` feature is a default feature and provides the `ToSchema` derive macro. The `uuid` feature enables `Uuid` type support in generated OpenAPI schemas, and the `chrono` feature enables `DateTime<Utc>` support. Both are required since `ArtifactMeta` uses these types.

## Approach

1. **Add `utoipa` dependency to `Cargo.toml`.** In `crates/anvilml-core/Cargo.toml`, add a new line under the existing `[dependencies]` section:
   ```toml
   utoipa = { version = "5.5.0", features = ["uuid", "chrono"] }
   ```
   Rationale: `utoipa` provides the `ToSchema` derive macro. The `uuid` and `chrono` features are needed because `ArtifactMeta` fields use `Uuid` and `DateTime<Utc>`, and the derive macro needs these features to generate correct OpenAPI schema types.

2. **Create `crates/anvilml-core/src/types/artifact.rs`.** Write the `ArtifactMeta` struct with all eight fields, doc comments on the struct and each field, and the required derives:
   ```rust
   use chrono::{DateTime, Utc};
   use serde::{Deserialize, Serialize};
   use std::path::PathBuf;
   use uuid::Uuid;
   use utoipa::ToSchema;

   /// Metadata for a generated, content-addressed PNG artifact.
   ///
   /// This struct captures the identity, generation parameters, and
   /// storage location of a single output artifact produced by a worker.
   /// The `hash` field is the SHA-256 hex content address used by
   /// `anvilml-artifacts` as its primary key.
   #[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
   pub struct ArtifactMeta {
       /// SHA-256 hex content address — primary key for artifact storage.
       pub hash: String,
       /// UUID of the job that produced this artifact.
       pub job_id: Uuid,
       /// Generated image width in pixels.
       pub width: u32,
       /// Generated image height in pixels.
       pub height: u32,
       /// Random seed used for generation (i64 to support negative seeds).
       pub seed: i64,
       /// Number of diffusion steps used in generation.
       pub steps: u32,
       /// Timestamp when the artifact was created.
       pub created_at: DateTime<Utc>,
       /// Filesystem path to the saved PNG file.
       pub file_path: PathBuf,
   }
   ```
   Rationale: The derive set matches the pattern in `job.rs` (Debug, Clone, Serialize, Deserialize) plus `ToSchema`. `PartialEq`/`Eq` are not derived here because `PathBuf` comparison is not needed for artifact metadata (the hash serves as the identity key). This matches the pattern: `ModelMeta` derives `PartialEq, Eq` because it's compared in tests, but `ArtifactMeta` is only constructed and serialized/deserialized, not compared.

3. **Update `crates/anvilml-core/src/types/mod.rs`.** Add one line declaring the artifact module and one line re-exporting it:
   ```rust
   pub mod artifact;
   pub use artifact::*;
   ```
   Rationale: Each task in Phase 3 adds exactly one `mod` declaration and one `pub use` to `types/mod.rs`, per the "Known Constraints and Gotchas" in `TASKS_PHASE003.md`. The module is declared alphabetically after `job` and `model` would be wrong — actually, alphabetically `artifact` comes before `job` and `model`. However, the task spec says "each task adds exactly one line there, never restructuring the file as a whole" — so we append at the end rather than inserting in alphabetical order.

4. **Create `crates/anvilml-core/tests/artifact_tests.rs`.** Write ≥3 tests following the established pattern from `model_tests.rs` and `job_tests.rs`:
   - **Test 1: `test_artifact_meta_serde_roundtrip`** — Construct an `ArtifactMeta` with all fields populated, serialise to JSON, deserialise back, assert equality. Also parse the JSON to verify field names.
   - **Test 2: `test_artifact_meta_hash_format`** — Verify that a SHA-256 hex hash (64 lowercase hex characters) roundtrips correctly through serde JSON.
   - **Test 3: `test_artifact_meta_field_names`** — Verify the JSON output contains the expected snake_case field names (`hash`, `job_id`, `width`, `height`, `seed`, `steps`, `created_at`, `file_path`).

## Public API Surface

| Item | Crate/Module Path | Description |
|------|-------------------|-------------|
| `pub struct ArtifactMeta` | `anvilml_core::types::ArtifactMeta` | Metadata for a generated artifact with 8 fields |

Full struct definition:
```rust
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ArtifactMeta {
    pub hash: String,
    pub job_id: Uuid,
    pub width: u32,
    pub height: u32,
    pub seed: i64,
    pub steps: u32,
    pub created_at: DateTime<Utc>,
    pub file_path: PathBuf,
}
```

Re-export via `types/mod.rs`:
```rust
pub mod artifact;
pub use artifact::*;
```

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | crates/anvilml-core/src/types/artifact.rs | ArtifactMeta struct definition |
| MODIFY | crates/anvilml-core/src/types/mod.rs | Add `mod artifact;` and `pub use artifact::*;` |
| MODIFY | crates/anvilml-core/Cargo.toml | Add `utoipa` dependency |
| CREATE | crates/anvilml-core/tests/artifact_tests.rs | ≥3 tests for construction and serde roundtrip |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `crates/anvilml-core/tests/artifact_tests.rs` | `test_artifact_meta_serde_roundtrip` | Full roundtrip: construct ArtifactMeta with all fields, serialise to JSON, deserialise back, assert equality | `cargo test -p anvilml-core --test artifact_tests -- test_artifact_meta_serde_roundtrip` exits 0 |
| `crates/anvilml-core/tests/artifact_tests.rs` | `test_artifact_meta_hash_format` | SHA-256 hex hash (64 lowercase hex chars) roundtrips correctly through serde JSON | `cargo test -p anvilml-core --test artifact_tests -- test_artifact_meta_hash_format` exits 0 |
| `crates/anvilml-core/tests/artifact_tests.rs` | `test_artifact_meta_field_names` | JSON output contains expected snake_case field names (hash, job_id, width, height, seed, steps, created_at, file_path) | `cargo test -p anvilml-core --test artifact_tests -- test_artifact_meta_field_names` exits 0 |

## CI Impact

No CI changes required. The new test file is picked up automatically by `cargo test --workspace --features mock-hardware` since it lives in the crate's `tests/` directory. No new file types, gates, or test modules are added beyond what the existing CI infrastructure already handles.

## Platform Considerations

None identified. The `ArtifactMeta` type is a pure data struct with no platform-specific behaviour. `PathBuf` handles platform path separators transparently via `serde`'s string serialisation. The `DateTime<Utc>` uses RFC 3339 format which is platform-neutral. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `utoipa` 5.5.0 derive macro may not compile with Rust edition 2024 (workspace pin) | Low | High | The `rust-docs_get_crate_version` shows MSRV 1.75 for utoipa 5.5.0; the workspace uses 1.96.0, well above the MSRV. The `macros` default feature provides `utoipa-gen` as a proc-macro dependency which compiles independently of edition. |
| `PathBuf` serialisation may differ between Rust and other languages | Low | Low | `serde`'s built-in `PathBuf` impl serialises as a UTF-8 string, which is the standard approach. The design doc expects this type to appear in HTTP responses (JSON), where string paths are the natural representation. |
| `ToSchema` derive requires `utoipa-gen` proc-macro which may add unexpected compile-time overhead | Low | Low | `utoipa` is a build-time-only dependency for the OpenAPI binary (`anvilml-openapi`). It does not affect runtime binary size. The proc-macro is already used in other types (Job, ModelMeta) that reference `ToSchema` in the design doc. |

## Acceptance Criteria

- [ ] `cargo check -p anvilml-core --features mock-hardware` exits 0
- [ ] `cargo test -p anvilml-core --test artifact_tests` exits 0 with ≥3 tests
- [ ] `head -1 .forge/reports/P3-A3_plan.md` prints `# Plan Report: P3-A3`
- [ ] `grep "^## " .forge/reports/P3-A3_plan.md` shows all 12 section headings
- [ ] `wc -l .forge/reports/P3-A3_plan.md` reports > 40 lines
