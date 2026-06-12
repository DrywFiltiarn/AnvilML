# Plan Report: P905-A5

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P905-A5                                           |
| Phase       | 905 — FP8 dtype + model metadata patching         |
| Description | anvilml-registry: ModelMetaPatch type and store patch_meta method |
| Depends on  | P905-A4                                           |
| Project     | anvilml                                           |
| Planned at  | 2026-06-12T12:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add a `ModelMetaPatch` partial-update type in `anvilml-core` and implement the `patch_meta` method on `ModelRegistry` in `anvilml-registry` that applies optional `dtype_hint` and `kind` fields to an existing model record, recomputes `vram_estimate_mib` from the (possibly-changed) dtype, and returns the updated metadata.

## Scope

### In Scope
- Add `ModelMetaPatch { dtype_hint: Option<DType>, kind: Option<ModelKind> }` struct in `crates/anvilml-core/src/types/model.rs` with derives: `Debug`, `Clone`, `Deserialize`, `ToSchema`
- Re-export `ModelMetaPatch` from `crates/anvilml-core/src/lib.rs`
- Add `async fn patch_meta(&self, id: &str, patch: ModelMetaPatch) -> Result<Option<ModelMeta>>` method in `crates/anvilml-registry/src/store.rs`
- Add unit test `patch_meta_updates_dtype_recomputes_vram` in `crates/anvilml-registry/tests/patch_meta.rs`
- Bump `anvilml-registry` patch version in `Cargo.toml` (0.1.3 → 0.1.4)

### Out of Scope
- HTTP handler for PATCH /v1/models/:id (covered by P905-A6)
- OpenAPI server-side wire (covered by P905-A6)
- Any changes to `anvilml-server` crate
- Changes to existing tests in other files

## Approach

1. **Add `ModelMetaPatch` to `anvilml-core/src/types/model.rs`**
   - Define struct after `ModelMeta`:
     ```rust
     /// Partial update for model metadata.
     #[derive(Debug, Clone, Deserialize, ToSchema)]
     pub struct ModelMetaPatch {
         #[serde(default)]
         pub dtype_hint: Option<DType>,
         #[serde(default)]
         pub kind: Option<ModelKind>,
     }
     ```
   - Both fields are `Option<T>` so absent fields are naturally no-ops during deserialization.
   - `ToSchema` is required because the type will be used as a request body in the server handler (P905-A6).

2. **Re-export from `anvilml-core/src/lib.rs`**
   - Add `pub use types::model::ModelMetaPatch;` alongside existing model re-exports.

3. **Add `patch_meta` to `anvilml-registry/src/store.rs`**
   - Signature: `pub async fn patch_meta(&self, id: &str, patch: ModelMetaPatch) -> Result<Option<ModelMeta>, AnvilError>`
   - Algorithm:
     a. `let current = self.get(id).await?;`
     b. If `current.is_none()`, return `Ok(None)`.
     c. Clone `current` into `mut updated`.
     d. Apply `patch.dtype_hint`: if `Some(dt)`, set `updated.dtype_hint = dt`.
     e. Apply `patch.kind`: if `Some(k)`, set `updated.kind = k`.
     f. Recompute VRAM: `updated.vram_estimate_mib = scanner::vram_estimate_mib(updated.size_bytes, updated.dtype_hint);`
     g. Upsert: `self.upsert(&updated).await?;`
     h. Return `Ok(Some(updated))`.
   - Add `use crate::scanner::vram_estimate_mib;` at the top of `store.rs` (or use `crate::scanner::vram_estimate_mib` inline).

4. **Add test file `crates/anvilml-registry/tests/patch_meta.rs`**
   - Test name: `patch_meta_updates_dtype_recomputes_vram`
   - Setup: create in-memory/temp SQLite DB, upsert a model with `dtype_hint = DType::F16`, `size_bytes = 6_700_000_000`, `vram_estimate_mib = 6_700` (approx).
   - Call `patch_meta` with `dtype_hint = Some(DType::F32)`.
   - Assert returned `Some` with updated dtype = `DType::F32` and recomputed vram = `13_400` (2.0x factor for F32 vs 1.0x for F16).
   - Use the same pattern as existing tests (`store_get.rs`): `tempfile::NamedTempFile` for DB path, `anvilml_registry::db::open`, `anvilml_registry::ModelRegistry::new`.

5. **Bump `anvilml-registry` version**
   - In `crates/anvilml-registry/Cargo.toml`, change `version = "0.1.3"` → `version = "0.1.4"`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/types/model.rs` | Add `ModelMetaPatch` struct with Debug, Clone, Deserialize, ToSchema derives |
| Modify | `crates/anvilml-core/src/lib.rs` | Re-export `ModelMetaPatch` |
| Modify | `crates/anvilml-registry/src/store.rs` | Add `patch_meta` async method |
| Modify | `crates/anvilml-registry/Cargo.toml` | Bump patch version 0.1.3 → 0.1.4 |
| Create | `crates/anvilml-registry/tests/patch_meta.rs` | Unit test: `patch_meta_updates_dtype_recomputes_vram` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `tests/patch_meta.rs` | `patch_meta_updates_dtype_recomputes_vram` | Patching dtype_hint on an existing model applies the change, recomputes vram_estimate_mib using the scanner's factor, and persists the updated record |

## CI Impact

This task modifies types in `anvilml-core` (adding a new struct with `ToSchema`) and adds logic in `anvilml-registry`. The `ToSchema` derive on `ModelMetaPatch` will cause `anvilml-openapi` to emit a new schema component, so the OpenAPI drift gate (`cargo run -p anvilml-openapi && git diff --exit-code backend/openapi.json`) will need to be run during implementation. If `backend/openapi.json` is stale, regenerate and stage it. The primary CI command `cargo test -p anvilml-registry` must exit 0.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ToSchema` on `ModelMetaPatch` causes OpenAPI drift gate failure | High | Requires regeneration of openapi.json | Run `cargo run -p anvilml-openapi` during implementation and stage the result |
| `vram_estimate_mib` function visibility — not pub in scanner module | Low | Medium | `vram_estimate_mib` is already `pub` in `scanner.rs` (line 120); verified by reading source |
| Test DB isolation conflict with other tests | Low | Medium | Use `tempfile::NamedTempFile` per test (same pattern as existing tests) — each test gets its own file |
| `serde(default)` on `Option<T>` fields — wrong deserialization behavior | None | None | `Option<T>` with `#[serde(default)]` correctly deserializes absent fields as `None` — standard Rust serde pattern |

## Acceptance Criteria

- [ ] `ModelMetaPatch` struct exists in `crates/anvilml-core/src/types/model.rs` with `dtype_hint: Option<DType>` and `kind: Option<ModelKind>`, derives `Debug`, `Clone`, `Deserialize`, `ToSchema`
- [ ] `ModelMetaPatch` is re-exported from `anvilml-core` crate root
- [ ] `patch_meta` method exists on `ModelRegistry` in `crates/anvilml-registry/src/store.rs` with correct signature and behavior (None→Ok(None), apply Some fields, recompute vram, upsert, return updated)
- [ ] Test file `crates/anvilml-registry/tests/patch_meta.rs` exists with `patch_meta_updates_dtype_recomputes_vram` test
- [ ] `anvilml-registry` Cargo.toml version bumped to 0.1.4
- [ ] `cargo test -p anvilml-registry --features mock-hardware` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
