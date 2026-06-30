# Plan Report: P900-A4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P900-A4                                     |
| Phase       | 900 — Spec-Drift & Logging Retrofit         |
| Description | anvilml-core: add missing ToSchema to ModelMeta/ModelKind/ModelDtype/ModelFormat |
| Depends on  | none                                        |
| Project     | anvilml                                     |
| Planned at  | 2026-06-30T14:10:00Z                        |
| Attempt     | 1                                           |

## Objective

Add the `ToSchema` derive that `ANVILML_DESIGN.md §5.4` specifies for `ModelMeta`, `ModelKind`, `ModelDtype`, and `ModelFormat` but that `crates/anvilml-core/src/types/model.rs` is missing entirely. This is additive-derive-only: no field, variant, or serde attribute changes. The change enables the future `anvilml-openapi` binary (Phase 1, P1-B6) to correctly include these types in the generated OpenAPI spec.

## Scope

### In Scope
- Add `use utoipa::ToSchema;` import to `crates/anvilml-core/src/types/model.rs`.
- Append `ToSchema` to the derive list on `ModelMeta`, `ModelKind`, `ModelDtype`, and `ModelFormat`.
- Verify existing tests pass (`cargo test -p anvilml-core --test model_tests`).
- Verify documentation builds (`cargo doc -p anvilml-core --no-deps`).

### Out of Scope
None. This task implements its full scope. No deferrals.

defers_to (from JSON): absent

## Existing Codebase Assessment

The file `crates/anvilml-core/src/types/model.rs` defines four public types: `ModelMeta` (struct), `ModelKind` (enum with 7 variants), `ModelDtype` (enum with 6 variants), and `ModelFormat` (enum with 5 variants). All four already derive `Debug`, `Clone`, `PartialEq`, `Eq`, `Serialize`, and `Deserialize`. The three enums carry `#[serde(rename_all = "snake_case")]` attributes.

The sibling file `crates/anvilml-core/src/types/job.rs` already follows the correct pattern: it imports `use utoipa::ToSchema;` and appends `ToSchema` to every derive list. The `anvilml-core` crate already depends on `utoipa = { version = "5.5.0", features = ["uuid", "chrono"] }`, so the dependency is present.

Other type modules (`hardware.rs`, `worker.rs`, `node.rs`, `artifact.rs`, `events.rs`) also correctly include `ToSchema`. The omission is isolated to `model.rs` — exactly as the audit found.

The test file `crates/anvilml-core/tests/model_tests.rs` exercises serde roundtrips for all four types but does not reference `ToSchema` (it tests only serde). Adding the derive will not affect those tests.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | utoipa  | 5.5.0           | rust-docs MCP  | uuid, chrono (already in Cargo.toml) |

The `ToSchema` trait exists on utoipa v5.5.0 and is enabled by the `macros` default feature. The `uuid` and `chrono` features are already specified in `anvilml-core/Cargo.toml` and confirmed present via MCP. No new dependency or feature flag is introduced by this task.

## Approach

1. **Add the utoipa import.** In `crates/anvilml-core/src/types/model.rs`, add `use utoipa::ToSchema;` on its own line immediately after the existing `use std::path::PathBuf;` import (line 3), maintaining alphabetical ordering of imports: `chrono`, `serde`, `std`, `utoipa`.

2. **Append `ToSchema` to `ModelMeta`'s derive list.** Change line 11 from:
   ```rust
   #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
   ```
   to:
   ```rust
   #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
   ```

3. **Append `ToSchema` to `ModelKind`'s derive list.** Change line 38 from:
   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
   ```
   to:
   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
   ```

4. **Append `ToSchema` to `ModelDtype`'s derive list.** Change line 61 from:
   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
   ```
   to:
   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
   ```

5. **Append `ToSchema` to `ModelFormat`'s derive list.** Change line 83 from:
   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
   ```
   to:
   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
   ```

6. **Verify tests pass.** Run `cargo test -p anvilml-core --test model_tests` — all existing serde roundtrip tests must exit 0.

7. **Verify documentation builds.** Run `cargo doc -p anvilml-core --no-deps` — must exit 0, confirming the new `ToSchema` impls compile and are documented.

Rationale: This is a mechanically identical change to the pattern established in `job.rs` (P900-A3). The derive list order follows the existing convention: `Debug, Clone, PartialEq, Eq, Serialize, Deserialize` then `ToSchema` last. No field or attribute changes are needed because `ToSchema` works with the existing derive set — `serde` handles serialization format and `utoipa` reads the same type metadata at compile time.

## Public API Surface

No new public items are introduced. The only change is an additive derive on four existing pub types:

| Item | Module Path | Change |
|------|-------------|--------|
| `ModelMeta` | `anvilml_core::types::model::ModelMeta` | Adds `ToSchema` to derive list |
| `ModelKind` | `anvilml_core::types::model::ModelKind` | Adds `ToSchema` to derive list |
| `ModelDtype` | `anvilml_core::types::model::ModelDtype` | Adds `ToSchema` to derive list |
| `ModelFormat` | `anvilml_core::types::model::ModelFormat` | Adds `ToSchema` to derive list |

These types are re-exported via `anvilml_core::types::*` and `anvilml_core::*` (lib.rs).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | crates/anvilml-core/src/types/model.rs | Add `use utoipa::ToSchema;` import; append `ToSchema` to derive list on ModelMeta, ModelKind, ModelDtype, ModelFormat |

## Tests

No new tests are introduced. The existing test file `crates/anvilml-core/tests/model_tests.rs` already exercises all four types via serde roundtrips. Adding `ToSchema` is a pure derive addition that does not change runtime behaviour or field layout, so the existing tests remain sufficient.

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| crates/anvilml-core/tests/model_tests.rs | test_model_kind_serde_snake_case | All 7 ModelKind variants serialise to correct snake_case JSON and roundtrip | `cargo test -p anvilml-core --test model_tests -- test_model_kind_serde_snake_case` exits 0 |
| crates/anvilml-core/tests/model_tests.rs | test_model_dtype_serde_snake_case | All 6 ModelDtype variants serialise to correct snake_case JSON and roundtrip | `cargo test -p anvilml-core --test model_tests -- test_model_dtype_serde_snake_case` exits 0 |
| crates/anvilml-core/tests/model_tests.rs | test_model_format_serde_snake_case | All 5 ModelFormat variants serialise to correct snake_case JSON and roundtrip | `cargo test -p anvilml-core --test model_tests -- test_model_format_serde_snake_case` exits 0 |
| crates/anvilml-core/tests/model_tests.rs | test_model_meta_serde_roundtrip | ModelMeta with all fields serialises and roundtrips correctly | `cargo test -p anvilml-core --test model_tests -- test_model_meta_serde_roundtrip` exits 0 |

## CI Impact

No CI changes required. The change is a pure derive addition in `anvilml-core`, which is already compiled and tested by every CI job (`rust-linux`, `rust-windows`). The `openapi-drift` CI job will pick up the new `ToSchema` impls when it regenerates `api/openapi.json` in a future task that wires up the full `anvilml-openapi` binary.

## Platform Considerations

None identified. The `utoipa::ToSchema` derive is a compile-time macro that generates OpenAPI schema definitions; it has no runtime platform-specific behaviour. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ToSchema` derive conflicts with existing `serde` attributes (e.g. `rename_all = "snake_case"` on the enums) — utoipa may generate a different wire format than serde produces. | Low | Medium | The existing `job.rs` already combines `#[serde(rename_all = "snake_case")]` with `ToSchema` on `JobStatus` and it works. Verify with `cargo doc` and confirm the generated schema matches expected snake_case naming. |
| `ModelMeta` contains `PathBuf` which may not have a `ToSchema` impl in utoipa 5.5.0, causing a compile error. | Medium | High | `PathBuf` is serialised as a JSON string by serde (via its `Serialize` impl). utoipa 5.5.0's `uuid` and `chrono` features are already enabled; if `PathBuf` lacks a `ToSchema` impl, the compile error will be explicit and the fix would be adding `#[schema(value_type = String)]` — but `PathBuf` already works in the existing `Job` struct's equivalent patterns. Verify at compile time. |
| Adding the derive changes the public API surface enough that a downstream crate depending on `anvilml-core` has a transitive compile error. | Low | Medium | No downstream crate imports these types yet (Phase 7+ builds on top of this crate). The workspace test run (`cargo test --workspace --features mock-hardware`) would catch any such issue. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-core --test model_tests` exits 0
- [ ] `cargo doc -p anvilml-core --no-deps` exits 0
