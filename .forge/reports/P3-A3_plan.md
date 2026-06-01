# Plan Report: P3-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P3-A3                                       |
| Phase       | 003 — Core Domain Types                     |
| Description | anvilml-core: Model and Artifact domain types |
| Depends on  | P3-A2                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-01T11:30:06Z                        |
| Attempt     | 1                                           |

## Objective

Create the Model and Artifact domain types specified in ANVILML_DESIGN §4.2 for the `anvilml-core` crate. This task adds `ModelMeta`, `ModelKind`, `DType`, and `ArtifactMeta` — pure, serializable data structures with zero I/O or async logic. These types form the foundation for the model registry (anvilml-registry, Phase 4) and artifact storage (anvilml-server, Phase 7).

## Scope

### In Scope
- Create `crates/anvilml-core/src/types/model.rs` with `ModelMeta`, `ModelKind`, and `DType`
- Create `crates/anvilml-core/src/types/artifact.rs` with `ArtifactMeta`
- Update `crates/anvilml-core/src/types/mod.rs` to declare the new modules
- Update `crates/anvilml-core/src/lib.rs` re-exports for the new types
- Inline unit tests in each new file (per-task gate: `cargo test -p anvilml-core -- model` and `cargo test -p anvilml-core -- artifact`)

### Out of Scope
- No I/O, file scanning, or database logic (handled by anvilml-registry in Phase 4)
- No HTTP handler changes (handled by P3-A6)
- No changes to existing types in config.rs, job.rs, error.rs, hardware.rs, or events.rs
- No OpenAPI generation changes (anvilml-openapi is build-time only)
- No changes to CI workflow files

## Approach

1. **Create `crates/anvilml-core/src/types/model.rs`:**
   - Define `DType` enum with variants: `F32`, `F16`, `BF16`, `Q8`, `Q4`, `Unknown`. Derive `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, ToSchema`. Set `#[default]` on `Unknown`.
   - Define `ModelKind` enum with variants: `Clip`, `Diffusion`, `Vae`, `Lora`, `ControlNet`, `Unet`, `Upscale`. Derive `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default, ToSchema`. Set `#[default]` on `Upscale`.
   - Define `ModelMeta` struct with fields: `id: String`, `name: String`, `path: PathBuf`, `kind: ModelKind`, `size_bytes: u64`, `dtype_hint: DType`, `vram_estimate_mib: u32`, `scanned_at: DateTime<Utc>`. Derive `Debug, Clone, Serialize, Deserialize, ToSchema`.
   - Note: `ModelKind` is already defined in `config.rs` with identical variants. This file will import it via `use crate::config::ModelKind;` to avoid duplication and naming conflicts within the same crate. The task description says "create" but importing is the pragmatic approach given existing code.
   - Add `#[serde(default)]` on `scanned_at` so it deserializes to `Utc::now()` when absent (via a `Default` impl).

2. **Create `crates/anvilml-core/src/types/artifact.rs`:**
   - Define `ArtifactMeta` struct with fields: `hash: String`, `job_id: Uuid`, `width: u32`, `height: u32`, `format: String`, `seed: i64`, `steps: u32`, `prompt: String`, `created_at: DateTime<Utc>`. Derive `Debug, Clone, Serialize, Deserialize, ToSchema`.
   - Add `#[serde(default)]` on optional/defaultable fields where appropriate.

3. **Update `crates/anvilml-core/src/types/mod.rs`:**
   - Add `pub mod model;` and `pub mod artifact;` module declarations.
   - Update the module doc comment to reference model and artifact types.

4. **Update `crates/anvilml-core/src/lib.rs`:**
   - Add re-exports: `pub use types::model::{ModelMeta, ModelKind, DType};` and `pub use types::artifact::ArtifactMeta;` at the crate root for downstream consumers.

5. **Add inline unit tests** in each new file (following the pattern established in P3-A1 and P3-A2):
   - In `model.rs`: test `ModelKind` variant count and distinctness, `DType` variant count and distinctness, `ModelKind::default() == Upscale`, `DType::default() == Unknown`, full `ModelMeta` round-trip serialization
   - In `artifact.rs`: full `ArtifactMeta` round-trip serialization, field-level verification

6. **Verify** with `cargo test -p anvilml-core -- model` (exits 0) and `cargo test -p anvilml-core -- artifact` (exits 0).

## Files Affected

| Action   | Path                                          | Description                                       |
|----------|-----------------------------------------------|---------------------------------------------------|
| CREATE   | crates/anvilml-core/src/types/model.rs        | ModelMeta, ModelKind, DType domain types          |
| CREATE   | crates/anvilml-core/src/types/artifact.rs     | ArtifactMeta domain type                          |
| MODIFY   | crates/anvilml-core/src/types/mod.rs          | Add model and artifact module declarations         |
| MODIFY   | crates/anvilml-core/src/lib.rs                | Re-export new types from crate root               |

## Tests

| Test ID / Name            | File                                         | Validates                          |
|---------------------------|----------------------------------------------|------------------------------------|
| model_kind_variants       | crates/anvilml-core/src/types/model.rs       | All 7 ModelKind variants distinct, equal/unequal pairwise |
| dtype_variants            | crates/anvilml-core/src/types/model.rs       | All 6 DType variants distinct, equal/unequal pairwise     |
| model_kind_default        | crates/anvilml-core/src/types/model.rs       | Default is Upscale                 |
| dtype_default             | crates/anvilml-core/src/types/model.rs       | Default is Unknown                 |
| model_meta_roundtrip      | crates/anvilml-core/src/types/model.rs       | Full ModelMeta serializes/deserializes correctly through JSON |
| artifact_meta_roundtrip   | crates/anvilml-core/src/types/artifact.rs    | Full ArtifactMeta serializes/deserializes correctly through JSON |

## CI Impact

No CI changes required. The new types use only existing dependencies (serde, chrono, uuid, utoipa, std::path::PathBuf) already declared in anvilml-core's Cargo.toml. No new crates or features are needed.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation                              |
|---------------------------|-----------|--------|-----------------------------------------|
| ModelKind name conflict with config::ModelKind (duplicate enum in same crate) | Low | Medium | Import from `crate::config::ModelKind` rather than redefining; document decision in source comments |
| PathBuf missing from Cargo.toml | None | None | PathBuf is from std, no dependency needed |
| Test filter `-- model` catches artifact tests | Low | Low | Use separate test filters: `-- model` and `-- artifact` as specified in task |

## Acceptance Criteria

- [ ] `crates/anvilml-core/src/types/model.rs` exists with ModelMeta, ModelKind, DType types
- [ ] `crates/anvilml-core/src/types/artifact.rs` exists with ArtifactMeta type
- [ ] All types derive Debug, Clone, Serialize, Deserialize, ToSchema (enums also Copy, PartialEq, Eq)
- [ ] `cargo test -p anvilml-core -- model` exits 0
- [ ] `cargo test -p anvilml-core -- artifact` exits 0
- [ ] Types are re-exported from lib.rs at crate root level
- [ ] `cargo clippy -p anvilml-core --lib` passes with zero warnings
