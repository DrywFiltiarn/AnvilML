# Plan Report: P3-A2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P3-A2                                             |
| Phase       | 003 — Core Domain Types: Data Model               |
| Description | anvilml-core: ModelMeta, ModelKind, ModelDtype, ModelFormat |
| Depends on  | P3-A1                                             |
| Project     | anvilml                                           |
| Planned at  | 2026-06-28T15:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create the model metadata types (`ModelMeta`, `ModelKind`, `ModelDtype`, `ModelFormat`)
in `crates/anvilml-core/src/types/model.rs` per `ANVILML_DESIGN.md §5.4`, declare the
`model` submodule in `types/mod.rs`, and add ≥4 integration tests in
`crates/anvilml-core/tests/model_tests.rs` that verify each enum serialises to its
expected `snake_case` JSON string via serde.

## Scope

### In Scope
- Create `crates/anvilml-core/src/types/model.rs` with:
  - `ModelMeta` struct: `id: String`, `name: String`, `path: PathBuf`,
    `kind: ModelKind`, `dtype: ModelDtype`, `format: ModelFormat`,
    `size_bytes: u64`, `scanned_at: DateTime<Utc>`; derives:
    `Debug, Clone, Serialize, Deserialize`.
  - `ModelKind` enum: variants `Diffusion, TextEncoder, Vae, Lora, ControlNet, Upscale, Unknown`;
    derives: `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize`;
    `#[serde(rename_all = "snake_case")]`.
  - `ModelDtype` enum: variants `Fp32, Fp16, Bf16, Fp8, Fp4, Unknown`;
    derives: `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize`;
    `#[serde(rename_all = "snake_case")]`.
  - `ModelFormat` enum: variants `Safetensors, Ckpt, Pt, Bin, Unknown`;
    derives: `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize`;
    `#[serde(rename_all = "snake_case")]`.
  - `///` doc comments on all public items (struct and every enum).
- Add `pub mod model;` to `crates/anvilml-core/src/types/mod.rs`.
- Create `crates/anvilml-core/tests/model_tests.rs` with ≥4 tests.
- Bump `anvilml-core` patch version in `Cargo.toml` (0.1.6 → 0.1.7).

### Out of Scope
defers_to (from JSON): []

No scope is deferred. This task implements its full scope in full.

## Existing Codebase Assessment

No prior source exists for `model.rs` — this file will be created from scratch.

The existing codebase provides established patterns to follow:
- **`crates/anvilml-core/src/types/job.rs`** — the closest precedent. It defines
  `Job`, `JobStatus`, and `JobSettings` with the same derive conventions
  (`Debug, Clone, Copy` where appropriate, `Serialize, Deserialize`, `serde(rename_all)`).
  Doc comments use `///` on every public item with a one-sentence summary.
- **`crates/anvilml-core/tests/job_tests.rs`** — the test style pattern: integration
  tests in `tests/` that import via `anvilml_core::types::*`, construct values,
  serialise to JSON, roundtrip, and assert equality. Tests have `///` doc comments
  describing what they verify.
- **`crates/anvilml-core/src/types/mod.rs`** — currently 3 lines with `pub mod job;`
  and `pub use job::*;`. Each new type module adds exactly one `pub mod <name>;` line
  (per `TASKS_PHASE003.md` "Known Constraints").
- **`crates/anvilml-core/Cargo.toml`** — already has `chrono` with `serde` feature
  (needed for `DateTime<Utc>`), `serde` with `derive` feature, and `serde_json` for
  test assertions. No new dependency is required.

The design doc (`ANVILML_DESIGN.md §5.4`) shows `ToSchema` on all types, but the
current codebase (job.rs) omits it — `utoipa` is not yet in `Cargo.toml` (it is added
in P3-A3 for `ArtifactMeta`). This task follows the existing pattern and omits
`ToSchema`, matching the task context's explicit derive list.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source | Feature flags confirmed |
|--------|---------|-----------------|------------|------------------------|
| crate  | chrono  | 0.4 (already in Cargo.toml) | lockfile | serde |

No new external crates are introduced. `chrono` with the `serde` feature is already
declared in `anvilml-core/Cargo.toml` and provides `DateTime<Utc>`.

## Approach

1. **Read existing patterns.** Read `job.rs` and `job_tests.rs` to confirm the naming,
   derive, and doc-comment conventions used in this crate's types module. (Already done
   during codebase inspection.)

2. **Create `crates/anvilml-core/src/types/model.rs`.** Write the file with:
   - Imports: `use chrono::DateTime; use chrono::Utc; use serde::{Deserialize, Serialize}; use std::path::PathBuf;`
   - `ModelMeta` struct (not `Copy` — it owns a `PathBuf`):
     ```rust
     /// Metadata about a discovered model file.
     #[derive(Debug, Clone, Serialize, Deserialize)]
     pub struct ModelMeta {
         /// Stable identifier: SHA256 hex of the first 1 MiB of the file.
         pub id: String,
         /// Human-readable model name.
         pub name: String,
         /// Filesystem path to the model file.
         pub path: PathBuf,
         /// The model's architecture family.
         pub kind: ModelKind,
         /// The model's data type / precision.
         pub dtype: ModelDtype,
         /// The model file format.
         pub format: ModelFormat,
         /// File size in bytes.
         pub size_bytes: u64,
         /// Timestamp when this metadata was scanned.
         pub scanned_at: DateTime<Utc>,
     }
     ```
   - `ModelKind` enum (with `Copy`):
     ```rust
     /// The architecture family of a model file.
     #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
     #[serde(rename_all = "snake_case")]
     pub enum ModelKind {
         Diffusion,
         TextEncoder,
         Vae,
         Lora,
         ControlNet,
         Upscale,
         Unknown,
     }
     ```
   - `ModelDtype` enum (with `Copy`):
     ```rust
     /// The data type or precision of a model's weights.
     #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
     #[serde(rename_all = "snake_case")]
     pub enum ModelDtype {
         Fp32,
         Fp16,
         Bf16,
         Fp8,
         Fp4,
         Unknown,
     }
     ```
   - `ModelFormat` enum (with `Copy`):
     ```rust
     /// The storage format of a model file.
     #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
     #[serde(rename_all = "snake_case")]
     pub enum ModelFormat {
         Safetensors,
         Ckpt,
         Pt,
         Bin,
         Unknown,
     }
     ```
   - Note: `PathBuf` serialises as a UTF-8 string via serde's built-in implementation —
     no special `#[serde(...)]` attribute is needed.

3. **Add `pub mod model;` to `types/mod.rs`.** Append one line:
   ```rust
   pub mod model;
   ```
   This follows the established convention: each task adds exactly one `pub mod <name>;`
   line, never restructuring the file.

4. **Create `crates/anvilml-core/tests/model_tests.rs`.** Write ≥4 integration tests
   following the `job_tests.rs` pattern: import via `anvilml_core::types::*`, construct
   values, serialise to JSON, verify the `snake_case` output, roundtrip, assert equality.
   Tests:
   - `test_model_kind_serde_snake_case` — serialise all 7 `ModelKind` variants, verify
     each produces the expected `"snake_case"` JSON string (e.g. `"text_encoder"`),
     and roundtrip back to equal.
   - `test_model_dtype_serde_snake_case` — serialise all 6 `ModelDtype` variants,
     verify each produces the expected `"snake_case"` JSON string (e.g. `"bf16"`),
     and roundtrip back to equal.
   - `test_model_format_serde_snake_case` — serialise all 5 `ModelFormat` variants,
     verify each produces the expected `"snake_case"` JSON string (e.g. `"safetensors"`),
     and roundtrip back to equal.
   - `test_model_meta_serde_roundtrip` — construct a full `ModelMeta` with all fields
     populated, serialise to JSON, roundtrip, assert equality. This verifies that
     `PathBuf` → `String` roundtrip works correctly via serde's built-in impl.

5. **Bump `anvilml-core` patch version.** In `Cargo.toml`, change
   `version = "0.1.6"` → `version = "0.1.7"`. Only the patch digit changes.

## Public API Surface

| Item | Path | Description |
|------|------|-------------|
| `struct ModelMeta` | `anvilml_core::types::ModelMeta` | Model file metadata struct (8 fields) |
| `enum ModelKind` | `anvilml_core::types::ModelKind` | Architecture family enum (7 variants) |
| `enum ModelDtype` | `anvilml_core::types::ModelDtype` | Data type / precision enum (6 variants) |
| `enum ModelFormat` | `anvilml_core::types::ModelFormat` | File format enum (5 variants) |

All items are `pub` and re-exported through `types/mod.rs` via the implicit
`pub use model::*;` that comes from `pub mod model;` (Rust auto-re-exports items
declared in a `pub mod` — but since the existing pattern uses explicit `pub use`,
and `job.rs` is accessed via `pub mod job;` + `pub use job::*;`, the model items
will be accessible as `types::model::ModelMeta` etc. To match the existing pattern
where `types::*` re-exports everything (via `pub use types::*;` in `lib.rs`), the
model items will be accessible as `anvilml_core::types::model::ModelMeta`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/types/model.rs` | `ModelMeta`, `ModelKind`, `ModelDtype`, `ModelFormat` |
| MODIFY | `crates/anvilml-core/src/types/mod.rs` | Add `pub mod model;` |
| CREATE | `crates/anvilml-core/tests/model_tests.rs` | ≥4 integration tests |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Bump patch version 0.1.6 → 0.1.7 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-core/tests/model_tests.rs` | `test_model_kind_serde_snake_case` | All 7 `ModelKind` variants serialise to correct `snake_case` JSON strings and roundtrip | None | Construct each variant | JSON `"diffusion"`, `"text_encoder"`, `"vae"`, `"lora"`, `"control_net"`, `"upscale"`, `"unknown"`; roundtrip equals original | `cargo test -p anvilml-core --test model_tests` exits 0 |
| `crates/anvilml-core/tests/model_tests.rs` | `test_model_dtype_serde_snake_case` | All 6 `ModelDtype` variants serialise to correct `snake_case` JSON strings and roundtrip | None | Construct each variant | JSON `"fp32"`, `"fp16"`, `"bf16"`, `"fp8"`, `"fp4"`, `"unknown"`; roundtrip equals original | `cargo test -p anvilml-core --test model_tests` exits 0 |
| `crates/anvilml-core/tests/model_tests.rs` | `test_model_format_serde_snake_case` | All 5 `ModelFormat` variants serialise to correct `snake_case` JSON strings and roundtrip | None | Construct each variant | JSON `"safetensors"`, `"ckpt"`, `"pt"`, `"bin"`, `"unknown"`; roundtrip equals original | `cargo test -p anvilml-core --test model_tests` exits 0 |
| `crates/anvilml-core/tests/model_tests.rs` | `test_model_meta_serde_roundtrip` | Full `ModelMeta` struct serialises and roundtrips correctly including `PathBuf` → `String` conversion | None | Construct `ModelMeta` with all fields | Roundtripped struct equals original | `cargo test -p anvilml-core --test model_tests` exits 0 |

## CI Impact

No CI changes required. The test is a standard Rust integration test under
`crates/anvilml-core/tests/`, picked up automatically by `cargo test --workspace`
(which runs on every CI job). No new file types, gates, or test modules are introduced.

## Platform Considerations

None identified. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.
`PathBuf` serialises as a platform-native path string, but since the roundtrip test
uses a constructed path and asserts equality (not a file-system-dependent value),
there are no platform-specific branches or `#[cfg]` guards needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `PathBuf` serialisation produces platform-dependent path separators, causing the `ModelMeta` roundtrip test to fail on Windows (backslashes vs forward slashes) | Low | Medium | The roundtrip test uses a `PathBuf` constructed via `PathBuf::from("models/checkpoint.safetensors")` and asserts equality after roundtrip. Since serde's `PathBuf` impl serialises to a string and deserialises back to the same string, the roundtrip is identity — no platform-dependent conversion occurs. The test will pass on all platforms. |
| `utoipa::ToSchema` derive is expected by downstream consumers (handlers, OpenAPI generator) but not included here, causing a compilation error in a later task that references these types | Low | Low | The existing `Job` type (P3-A1) also omits `ToSchema` — this is the established pattern. `utoipa` is added in P3-A3 for `ArtifactMeta`. If a later task requires `ToSchema` on model types, it will add the dependency and derive at that time. |
| `#[serde(rename_all = "snake_case")]` on enums with single-word variants (e.g. `Fp32`) produces unexpected casing (`fp32` vs `fp_32`) | Low | Medium | Rust's `serde` `snake_case` correctly handles alphanumeric boundaries: `Fp32` → `"fp32"`, `Bf16` → `"bf16"`, `Vae` → `"vae"`. This is confirmed by serde's documented behaviour. The test assertions use the correct expected strings. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-core --test model_tests` exits 0
- [ ] `cargo fmt --all -- --check` exits 0 (format gate)
- [ ] `grep -c 'pub mod model;' crates/anvilml-core/src/types/mod.rs` outputs `1`
- [ ] `grep -c 'version = "0.1.7"' crates/anvilml-core/Cargo.toml` outputs `1`
