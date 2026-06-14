# Plan Report: P3-A2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P3-A2                                             |
| Phase       | 003 — Core Domain Types                           |
| Description | anvilml-core: model and artifact types            |
| Depends on  | P3-A1 (job types)                                 |
| Project     | anvilml                                           |
| Planned at  | 2026-06-14T14:52:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create the `ModelMeta`, `ModelKind`, `ModelDtype`, `ModelFormat` structs and enums in `crates/anvilml-core/src/types/model.rs`, and the `ArtifactMeta` struct in `crates/anvilml-core/src/types/artifact.rs`, per the type specifications in `ANVILML_DESIGN.md §5.4–5.5`. Wire these into the crate's module tree (`types/mod.rs`, `lib.rs`), bump the crate patch version, and add ≥3 tests each for the model and artifact modules under `crates/anvilml-core/tests/`. When complete, `cargo test -p anvilml-core -- types::model` and `cargo test -p anvilml-core -- types::artifact` both exit 0, and downstream crates can import `anvilml_core::{ModelMeta, ModelKind, ModelDtype, ModelFormat, ArtifactMeta}`.

## Scope

### In Scope
- **CREATE** `crates/anvilml-core/src/types/model.rs` — `ModelMeta` struct, `ModelKind` enum (Diffusion, TextEncoder, Vae, Lora, ControlNet, Upscale, Unknown), `ModelDtype` enum (Fp32, Fp16, Bf16, Fp8, Fp4, Unknown), `ModelFormat` enum (Safetensors, Ckpt, Pt, Bin, Unknown). All fields serde snake_case, all derive `Debug, Clone, Serialize, Deserialize, ToSchema`. Enums also derive `Copy, PartialEq, Eq`.
- **CREATE** `crates/anvilml-core/src/types/artifact.rs` — `ArtifactMeta` struct with `id`, `job_id`, `hash`, `path`, `size_bytes`, `created_at` fields. All fields serde snake_case, all derive `Debug, Clone, Serialize, Deserialize, ToSchema`.
- **MODIFY** `crates/anvilml-core/src/types/mod.rs` — add `pub mod model;` and `pub mod artifact;`, add corresponding `pub use` re-exports.
- **MODIFY** `crates/anvilml-core/src/lib.rs` — add `pub use types::{ModelMeta, ModelKind, ModelDtype, ModelFormat, ArtifactMeta};`.
- **MODIFY** `crates/anvilml-core/Cargo.toml` — bump patch version `0.1.3 → 0.1.4`.
- **CREATE** `crates/anvilml-core/tests/model_tests.rs` — ≥3 tests for model types (JSON roundtrip of ModelMeta, all ModelKind variants roundtrip, all ModelDtype and ModelFormat variants roundtrip).
- **CREATE** `crates/anvilml-core/tests/artifact_tests.rs` — ≥3 tests for artifact type (JSON roundtrip of ArtifactMeta, default impl verification, hash format validation).

### Out of Scope
- Model scanning logic (belongs to `anvilml-registry` scanner module — future task).
- Database schema for persisting `ModelMeta` / `ArtifactMeta` (belongs to `anvilml-registry` store — future task).
- HTTP handler types or response wrappers (belongs to `anvilml-server` — future task).
- Any `#[tracing::instrument]` annotations — these types are pure data with no I/O or decision points.

## Existing Codebase Assessment

The `anvilml-core` crate at version 0.1.3 already defines the job types (`Job`, `JobStatus`, `JobSettings`, `SubmitJobRequest`, `SubmitJobResponse`) in `src/types/job.rs` with full `Serialize, Deserialize, ToSchema` derives, doc comments on every pub item, and comprehensive integration tests in `tests/job_tests.rs`. The `types/mod.rs` declares `pub mod job;` and re-exports the types. The `lib.rs` re-exports from both `config` and `types` modules.

Established patterns to follow:
- **Derives**: Every struct derives `Debug, Clone, Serialize, Deserialize, ToSchema`. Enums also derive `Copy, PartialEq, Eq`. The `ToSchema` derive from `utoipa` is used for OpenAPI schema generation.
- **Doc comments**: Every pub item has a `///` doc comment describing what it does. Struct fields have inline doc comments.
- **Serde**: Enums use `#[serde(rename_all = "snake_case")]`. Structs rely on default serde field naming (which is already snake_case for the defined fields).
- **Test style**: Integration tests live in `crates/anvilml-core/tests/` as separate test crates. Tests import via `use anvilml_core::{...}`. Each test has a `///` doc comment explaining what it verifies. Tests use `serde_json::to_string` / `serde_json::from_str` for roundtrip verification.
- **Version bump**: The crate's patch version is incremented for every task that modifies source files.

No discrepancies between the design doc (§5.4) and current source — the model types simply do not exist yet. The job types serve as the template for implementation style.

## Resolved Dependencies

All dependencies are already declared in the workspace `Cargo.toml`. No new external crates are introduced.

| Type   | Name     | Version verified | MCP source     | Feature flags confirmed |
|--------|----------|-----------------|----------------|------------------------|
| crate  | serde    | 1.0.228         | Cargo.lock fallback (rust-docs MCP unavailable) | derive |
| crate  | chrono   | 0.4.45          | Cargo.lock fallback | serde |
| crate  | uuid     | 1.23.3          | Cargo.lock fallback | serde, v4 |
| crate  | utoipa   | 5.5.0           | Cargo.lock fallback | macros, chrono, uuid |

Note: The rust-docs MCP tool was unavailable for this session. All version numbers are taken from the workspace `Cargo.toml` which is the authoritative source for workspace dependency declarations. These versions are confirmed to exist in the project's committed state.

## Approach

1. **Create `crates/anvilml-core/src/types/model.rs`** with the four types from `ANVILML_DESIGN.md §5.4`:
   - `ModelMeta` struct: fields `id: String`, `name: String`, `path: PathBuf`, `kind: ModelKind`, `dtype: ModelDtype`, `format: ModelFormat`, `size_bytes: u64`, `scanned_at: DateTime<Utc>`. Derive `Debug, Clone, Serialize, Deserialize, ToSchema`. Add `///` doc comment on the struct and each field. Use `use std::path::PathBuf;` and `use chrono::{DateTime, Utc};` imports.
   - `ModelKind` enum: variants `Diffusion, TextEncoder, Vae, Lora, ControlNet, Upscale, Unknown`. Derive `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema`. Add `#[serde(rename_all = "snake_case")]` attribute. Add `///` doc comment on each variant.
   - `ModelDtype` enum: variants `Fp32, Fp16, Bf16, Fp8, Fp4, Unknown`. Same derives and serde attribute as ModelKind.
   - `ModelFormat` enum: variants `Safetensors, Ckpt, Pt, Bin, Unknown`. Same derives and serde attribute.
   - Rationale: Follow the exact type shapes from §5.4. The `#[serde(rename_all = "snake_case")]` on enums ensures JSON output like `"diffusion"` rather than `"Diffusion"`, matching the OpenAPI contract.

2. **Create `crates/anvilml-core/src/types/artifact.rs`** with `ArtifactMeta`:
   - Struct fields: `id: String`, `job_id: Uuid`, `hash: String`, `path: PathBuf`, `size_bytes: u64`, `created_at: DateTime<Utc>`. Derive `Debug, Clone, Serialize, Deserialize, ToSchema`. Add `///` doc comment on struct and each field.
   - Add `use chrono::{DateTime, Utc};` and `use uuid::Uuid;` imports.
   - Rationale: Matches the task specification exactly. `PathBuf` serialises to string by default via serde, which is the correct representation for a filesystem path in JSON.

3. **Modify `crates/anvilml-core/src/types/mod.rs`**:
   - Add `pub mod model;` after `pub mod job;`.
   - Add `pub mod artifact;` after `pub mod model;`.
   - Update the module doc comment to mention model and artifact types.
   - Add `pub use model::{ModelMeta, ModelKind, ModelDtype, ModelFormat};`.
   - Add `pub use artifact::ArtifactMeta;`.

4. **Modify `crates/anvilml-core/src/lib.rs`**:
   - Add `ModelMeta, ModelKind, ModelDtype, ModelFormat, ArtifactMeta` to the `pub use types::{...}` line.

5. **Bump crate version** in `crates/anvilml-core/Cargo.toml`:
   - Change `version = "0.1.3"` to `version = "0.1.4"`.

6. **Create `crates/anvilml-core/tests/model_tests.rs`** with ≥3 tests:
   - `test_model_meta_json_roundtrip`: Build a fully-populated `ModelMeta`, serialise to JSON, deserialise back, assert all fields equal. Verifies the primary correctness guarantee for the struct's serde derives.
   - `test_model_kind_variants`: Roundtrip all seven `ModelKind` variants through JSON, assert equality. Verifies `#[serde(rename_all = "snake_case")]` works correctly on the enum.
   - `test_model_dtype_format_variants`: Roundtrip all `ModelDtype` and `ModelFormat` variants through JSON, assert equality.

7. **Create `crates/anvilml-core/tests/artifact_tests.rs`** with ≥3 tests:
   - `test_artifact_meta_json_roundtrip`: Build a fully-populated `ArtifactMeta`, serialise to JSON, deserialise back, assert all fields equal.
   - `test_artifact_meta_default`: Derive `Default` on `ArtifactMeta` (via `#[derive(Default)]` on the struct with `#[default]` on `id` using `Uuid::default()`), verify that `ArtifactMeta::default()` produces a well-formed struct.
   - `test_artifact_hash_format`: Verify that a SHA256 hex hash (64 lowercase hex chars) roundtrips correctly through JSON — ensures the `hash: String` field serialises as expected.

## Public API Surface

Every new public item introduced by this task:

| Module Path | Item | Kind | Description |
|-------------|------|------|-------------|
| `anvilml_core::types::model::ModelMeta` | struct | Data struct with 8 fields: id, name, path, kind, dtype, format, size_bytes, scanned_at |
| `anvilml_core::types::model::ModelKind` | enum | 7 variants: Diffusion, TextEncoder, Vae, Lora, ControlNet, Upscale, Unknown |
| `anvilml_core::types::model::ModelDtype` | enum | 6 variants: Fp32, Fp16, Bf16, Fp8, Fp4, Unknown |
| `anvilml_core::types::model::ModelFormat` | enum | 5 variants: Safetensors, Ckpt, Pt, Bin, Unknown |
| `anvilml_core::types::artifact::ArtifactMeta` | struct | Data struct with 6 fields: id, job_id, hash, path, size_bytes, created_at |

Re-exported at crate root via `lib.rs`:
- `anvilml_core::ModelMeta`
- `anvilml_core::ModelKind`
- `anvilml_core::ModelDtype`
- `anvilml_core::ModelFormat`
- `anvilml_core::ArtifactMeta`

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/types/model.rs` | ModelMeta struct + ModelKind, ModelDtype, ModelFormat enums |
| CREATE | `crates/anvilml-core/src/types/artifact.rs` | ArtifactMeta struct |
| MODIFY | `crates/anvilml-core/src/types/mod.rs` | Add model and artifact modules + pub use re-exports |
| MODIFY | `crates/anvilml-core/src/lib.rs` | Re-export new types at crate root |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Bump patch version 0.1.3 → 0.1.4 |
| CREATE | `crates/anvilml-core/tests/model_tests.rs` | ≥3 tests for model types |
| CREATE | `crates/anvilml-core/tests/artifact_tests.rs` | ≥3 tests for artifact type |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-core/tests/model_tests.rs` | `test_model_meta_json_roundtrip` | Fully-populated `ModelMeta` roundtrips through JSON without data loss | None | ModelMeta with all fields set (including mixed Some/None and PathBuf) | Deserialised struct equals original | `cargo test -p anvilml-core -- types::model --test model_tests test_model_meta_json_roundtrip` exits 0 |
| `crates/anvilml-core/tests/model_tests.rs` | `test_model_kind_variants` | All 7 `ModelKind` enum variants roundtrip through JSON | None | All ModelKind variants | Each variant equals its deserialised form | `cargo test -p anvilml-core -- types::model --test model_tests test_model_kind_variants` exits 0 |
| `crates/anvilml-core/tests/model_tests.rs` | `test_model_dtype_format_variants` | All `ModelDtype` and `ModelFormat` variants roundtrip through JSON | None | All ModelDtype and ModelFormat variants | Each variant equals its deserialised form | `cargo test -p anvilml-core -- types::model --test model_tests test_model_dtype_format_variants` exits 0 |
| `crates/anvilml-core/tests/artifact_tests.rs` | `test_artifact_meta_json_roundtrip` | Fully-populated `ArtifactMeta` roundtrips through JSON without data loss | None | ArtifactMeta with all fields set | Deserialised struct equals original | `cargo test -p anvilml-core -- types::artifact --test artifact_tests test_artifact_meta_json_roundtrip` exits 0 |
| `crates/anvilml-core/tests/artifact_tests.rs` | `test_artifact_meta_default` | `ArtifactMeta::default()` produces a well-formed struct with zero/empty defaults | ArtifactMeta derives Default | None | Default struct has empty id, uuid::Uuid::default() for job_id, empty hash, etc. | `cargo test -p anvilml-core -- types::artifact --test artifact_tests test_artifact_meta_default` exits 0 |
| `crates/anvilml-core/tests/artifact_tests.rs` | `test_artifact_hash_format` | A SHA256 hex hash string (64 lowercase hex chars) roundtrips correctly through JSON | None | String of 64 lowercase hex characters | Deserialised string equals original | `cargo test -p anvilml-core -- types::artifact --test artifact_tests test_artifact_hash_format` exits 0 |

## CI Impact

No CI changes required. The new test files follow the established pattern of `crates/{name}/tests/` integration test files, which are automatically picked up by `cargo test --workspace --features mock-hardware` (the CI test command). No new CI jobs, gates, or configuration changes are needed.

## Platform Considerations

None identified. All types are pure data with no platform-specific behaviour. `PathBuf` serialises to a string via serde's default implementation, which is platform-neutral at the JSON level. The `chrono::DateTime<Utc>` type uses UTC timestamps which are platform-independent. The `uuid::Uuid` type serialises as a standard hex string. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `utoipa::ToSchema` derive on `PathBuf` field may not generate correct schema — `PathBuf` is a std type and utoipa may not have a built-in schema for it. If this causes a compile error, replace `PathBuf` with `String` for the path fields. | Low | Medium | The existing `Job` struct uses `serde_json::Value` (not PathBuf) and compiles fine. If utoipa fails on `PathBuf`, the ACT agent should change `path: PathBuf` to `path: String` in both structs, which is functionally equivalent for JSON transport and matches how paths are typically represented in API responses. |
| `serde_json::to_string` on `PathBuf` may produce platform-specific path separators on Windows (`\` vs `/`). Downstream consumers may expect forward slashes. | Low | Low | This is a concern only at runtime during scanning/persistence, not during serialisation. The JSON representation of `PathBuf` uses `Display`, which produces the OS-native format. For the plan, this is acceptable — the scanner (future task) will normalise paths. No mitigation needed at this type-definition level. |
| Task context names `DateTime<Utc>` but the actual import requires `use chrono::{DateTime, Utc};` — the angle-bracket syntax is a type annotation, not an import. | N/A | None | The approach step explicitly lists the correct import: `use chrono::{DateTime, Utc};`. This is consistent with the existing `job.rs` which uses the same pattern. |
| `ArtifactMeta` derives `Default` — the `PathBuf` field's default is `PathBuf::new()` (empty path), which is valid but may be semantically unexpected. | Low | Low | The default is used only as a test placeholder and for API scaffolding. The scanner will always populate real paths. Documented in the test comment. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-core -- types::model` exits 0 with ≥ 3 tests
- [ ] `cargo test -p anvilml-core -- types::artifact` exits 0 with ≥ 3 tests
- [ ] `cargo build -p anvilml-core` exits 0 (all new types compile correctly)
- [ ] `head -1 .forge/reports/P3-A2_plan.md` prints `# Plan Report: P3-A2`
- [ ] `grep "^## " .forge/reports/P3-A2_plan.md` shows exactly 11 section headings
- [ ] `wc -l .forge/reports/P3-A2_plan.md` reports > 40 lines
