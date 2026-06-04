# Plan Report: P6-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P6-A3                                       |
| Phase       | 006 — Model Registry                        |
| Description | anvilml-registry: ModelRegistry list (with kind filter) |
| Depends on  | P6-A2                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-04T01:25:00Z                        |
| Attempt     | 1                                           |

## Objective

Add a `list` method to `ModelRegistry` in `store.rs` that returns all scanned model metadata from the SQLite `models` table, with an optional `kind` filter and deterministic ordering by name ascending.

## Scope

### In Scope
- Add `async fn list(&self, kind: Option<ModelKind>) -> Result<Vec<ModelMeta>>` to `ModelRegistry` in `crates/anvilml-registry/src/store.rs`
- SQL: `SELECT id, name, path, kind, size_bytes, dtype_hint, vram_estimate_mib, scanned_at FROM models` with optional `WHERE kind = ?` and `ORDER BY name ASC`
- Add integration test file `crates/anvilml-registry/tests/store_list.rs` with three tests:
  1. Empty database returns empty vector
  2. After 3 upserts, list returns all 3 ordered by name
  3. Kind filter returns only matching models

### Out of Scope
- Any changes to `lib.rs` re-exports (the method is on the existing `ModelRegistry` type)
- Any handler or HTTP endpoint changes (handled in P6-A6, P6-A7)
- Rescan logic (handled in P6-A4)
- Schema changes — the migration `002_models.sql` already has the needed columns and an index on `kind`

## Approach

1. **Add `list` method to `store.rs`** following the established pattern used by `get`:
   - Use `sqlx::query_as` with the `ModelRow` tuple type already defined in the file
   - Build the query string dynamically: base SELECT with optional `WHERE kind = ?` appended when `kind.is_some()`
   - Bind the kind value as a JSON string (matching how `upsert` serialises `kind`)
   - Order by `name ASC`
   - Map each row through the same deserialization logic already used in `get`:
     - Parse `kind` from JSON → `ModelKind`
     - Parse `dtype_hint` from JSON → `DType`
     - Parse `scanned_at` from RFC3339 → `DateTime<Utc>`
   - Return `Vec<ModelMeta>`

2. **Create test file `tests/store_list.rs`** mirroring the structure of `tests/store_get.rs`:
   - Use the same `open_pool` helper pattern (inline or copy)
   - Test 1 (`test_list_empty_returns_empty_vec`): open a fresh DB, call `list(None)`, assert `.is_empty()`
   - Test 2 (`test_list_after_upserts_returns_ordered`): upsert 3 models with names "Zebra", "Alpha", "Mango" (different kinds), call `list(None)`, assert len is 3 and order is Alpha, Mango, Zebra
   - Test 3 (`test_list_kind_filter`): upsert 2 Diffusion + 1 Vae models, call `list(Some(ModelKind::Diffusion))`, assert len is 2 and all are Diffusion

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-registry/src/store.rs` | Add `list` method to `impl ModelRegistry` |
| Create | `crates/anvilml-registry/tests/store_list.rs` | Integration tests for `list` method |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `tests/store_list.rs` | `test_list_empty_returns_empty_vec` | `list(None)` on empty DB returns `[]` (empty `Vec`) |
| `tests/store_list.rs` | `test_list_after_upserts_returns_ordered` | After 3 upserts with unsorted names, `list(None)` returns all 3 ordered by name ASC |
| `tests/store_list.rs` | `test_list_kind_filter` | `list(Some(ModelKind::Diffusion))` returns only Diffusion models, excluding other kinds |

## CI Impact

No CI workflow changes required. The new test is an integration test under the existing crate (`anvilml-registry`) and will be picked up by the standard `cargo test -p anvilml-registry -- store_list` command. No new dependencies are needed — the task uses only `sqlx`, `serde_json`, and `chrono` which are already declared in `Cargo.toml`.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Query string building with optional WHERE clause may produce trailing whitespace or syntax issues | Use Rust string interpolation to build the base query, then `.to_string()` + conditional append. Test with a fresh DB first to validate SQL. |
| Kind filter comparison must match how `kind` is stored (JSON-serialized string) | Reuse the same `serde_json::to_string(&meta.kind)` serialization approach already used in `upsert`, so storage and query use identical JSON encoding. |
| Ordering by name ASC requires all names to be non-null | The schema defines `name TEXT NOT NULL`, so this is guaranteed at the DB level. No additional null handling needed. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-registry -- store_list` exits 0 with all 3 tests passing
- [ ] Empty database: `list(None)` returns empty vector
- [ ] After 3 upserts with distinct names, `list(None)` returns exactly 3 `ModelMeta` values ordered by name ASC
- [ ] `list(Some(ModelKind::Diffusion))` returns only models whose `kind` field equals `Diffusion`
- [ ] `cargo clippy --package anvilml-registry --features mock-hardware -- -D warnings` passes with no errors or warnings
