# Plan Report: P906-A1

| Field       | Value                                         |
|-------------|-----------------------------------------------|
| Task ID     | P906-A1                                       |
| Phase       | 906 — OpenAPI Spec Correctness Retrofit       |
| Description | anvilml-openapi: add missing ModelKind schema to component registration |
| Depends on  | P905-A6, P905-A7                              |
| Project     | anvilml                                       |
| Planned at  | 2026-06-12T13:51:00Z                          |
| Attempt     | 1                                             |

## Objective

Register the `ModelKind` schema in the `anvilml-openapi` generator so that `backend/openapi.json` no longer contains dangling `$ref` pointers to `#/components/schemas/ModelKind`.

## Scope

### In Scope
- Add `use anvilml_core::ModelKind;` import to `crates/anvilml-openapi/src/main.rs`
- Add `.schema("ModelKind", ModelKind::schema())` to the components builder chain in `crates/anvilml-openapi/src/main.rs`
- Bump `anvilml-openapi` patch version from `0.1.1` to `0.1.2` in `crates/anvilml-openapi/Cargo.toml`
- Verify `cargo build -p anvilml-openapi` exits 0
- Verify `cargo clippy -p anvilml-openapi -- -D warnings` exits 0

### Out of Scope
- Regenerating `backend/openapi.json` (owned by P906-A4)
- Any changes to `anvilml-core`, `anvilml-server`, or handler code
- Any other schema registration fixes (owned by P906-A2)
- Any BF16 rename changes (owned by P906-A3)

## Approach

1. **Read** `crates/anvilml-openapi/src/main.rs` to confirm current schema registrations (already done — lines 54–91 show the components builder with no `ModelKind` entry).

2. **Add import** — insert `use anvilml_core::ModelKind;` into the existing import block at line 19 (`use anvilml_core::{ ... }`) or as a separate line alongside the existing `anvilml_core` imports. Since `ModelKind` is re-exported at the crate root (`pub use types::model::ModelKind` in `lib.rs`), the import path is `use anvilml_core::ModelKind;`.

3. **Add schema registration** — insert `.schema("ModelKind", ModelKind::schema())` into the components builder chain. Place it logically near the other config-type schemas (e.g., after `DeviceType` / `EnumerationSource` / `CapabilitySource` on lines 61–63, since `ModelKind` is also a domain config enum).

4. **Bump version** — change `version = "0.1.1"` to `version = "0.1.2"` in `crates/anvilml-openapi/Cargo.toml`.

5. **Verify build** — run `cargo build -p anvilml-openapi` and confirm exit 0.

6. **Verify clippy** — run `cargo clippy -p anvilml-openapi -- -D warnings` and confirm exit 0.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-openapi/src/main.rs` | Add `use anvilml_core::ModelKind;` import and `.schema("ModelKind", ModelKind::schema())` to components builder |
| Modify | `crates/anvilml-openapi/Cargo.toml` | Bump patch version `0.1.1 → 0.1.2` |

## Tests

None. This task adds a schema registration; there is no new test file to write. The acceptance criteria are build + clippy gates.

## CI Impact

No CI workflow files are modified. The existing CI gates (`cargo clippy --workspace --features mock-hardware -- -D warnings`, `cargo test --workspace --features mock-hardware`) will continue to apply. Adding `ModelKind` to the schema components does not change any public handler signatures or `#[utoipa::path]` annotations, so the OpenAPI drift gate (Gate 2) is not triggered by this task alone — it will be triggered by P906-A4 which regenerates `backend/openapi.json`.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ModelKind` is not in scope for `anvilml-core`'s `pub use` re-export | Very Low | Build failure | Confirmed: `lib.rs` line 12 exports `ModelKind` at crate root (`pub use types::model::ModelKind`). |
| Adding `.schema()` to the builder chain causes a type mismatch | Low | Build failure | `ModelKind` derives `ToSchema` (same as `DeviceType`, `EnumerationSource`, etc.), so `.schema("ModelKind", ModelKind::schema())` follows the exact same pattern already used for all other types. |
| Version bump conflicts with a concurrent task | Low | Merge conflict | This is the only task modifying `anvilml-openapi/Cargo.toml` in Phase 906 (A2 does not modify it). |

## Acceptance Criteria

- [ ] `crates/anvilml-openapi/src/main.rs` contains `use anvilml_core::ModelKind;` import
- [ ] `crates/anvilml-openapi/src/main.rs` contains `.schema("ModelKind", ModelKind::schema())` in the components builder
- [ ] `crates/anvilml-openapi/Cargo.toml` version is `0.1.2`
- [ ] `cargo build -p anvilml-openapi` exits 0
- [ ] `cargo clippy -p anvilml-openapi -- -D warnings` exits 0
