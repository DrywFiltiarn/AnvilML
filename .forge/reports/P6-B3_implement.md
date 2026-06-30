# Implementation Report: P6-B3

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P6-B3                                       |
| Phase         | 006 — Model Registry & Artifacts            |
| Description | anvilml-artifacts: ArtifactStore::list by job_id |
| Implemented   | 2026-06-30T08:30:00Z                        |
| Status        | COMPLETE                                    |

## Summary

Implemented `ArtifactStore::list(&self, job_id: Option<Uuid>)` in `crates/anvilml-artifacts/src/store.rs`, completing the store's public API surface (save → get → list). The method queries the `artifacts` table and returns `Vec<ArtifactMeta>`, optionally filtered by job ID. A manual row-mapping helper (`map_row`) was added because `PathBuf` is not a native sqlx SQLite type, preventing use of `query_as::<_, ArtifactMeta>`. Three new tests verify filtered listing, unfiltered listing, and empty-table handling. The `anvilml-artifacts` crate version was bumped from 0.1.2 to 0.1.3, and `uuid` was added as a regular dependency (previously only a dev-dependency) to support UUID parsing in production code.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source         |
|--------|---------|------------------|----------------|
| crate  | sqlx    | 0.9.0            | Cargo.toml (workspace) |
| crate  | uuid    | 1.23             | Cargo.toml (workspace) |

No new dependencies were introduced. The `uuid` dependency was moved from `[dev-dependencies]` to `[dependencies]` in `Cargo.toml` because it is required by the `map_row` helper in production code (UUID parsing from TEXT column). The `uuid` feature was also added to the `sqlx` dependency to enable UUID type support.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-artifacts/src/store.rs` | Added `list()` method (~70 lines) and `map_row()` helper (~25 lines); added `use uuid::Uuid` and `use sqlx::Row` imports |
| MODIFY | `crates/anvilml-artifacts/tests/store_tests.rs` | Added 3 new test functions: `test_list_with_job_id_filter`, `test_list_without_filter_returns_all`, `test_list_empty_table_returns_empty_vec` (~140 lines) |
| MODIFY | `crates/anvilml-artifacts/Cargo.toml` | Added `uuid` to `[dependencies]`; added `uuid` feature to `sqlx`; bumped version 0.1.2 → 0.1.3 |
| MODIFY | `docs/TESTS.md` | Added 3 test entries for the new `list()` tests |

## Commit Log

```
 Cargo.lock                                    |   6 +-
 crates/anvilml-artifacts/Cargo.toml           |   5 +-
 crates/anvilml-artifacts/src/store.rs         |  98 +++++++++++++++
 crates/anvilml-artifacts/tests/store_tests.rs | 167 ++++++++++++++++++++++++++
 docs/TESTS.md                                 |  36 ++++++
 5 files changed, 309 insertions(+), 3 deletions(-)
```

## Test Results

```
running 9 tests
test test_get_unknown_hash_returns_none ... ok
test test_list_empty_table_returns_empty_vec ... ok
test test_duplicate_save_does_not_duplicate_or_error ... ok
test test_save_then_get_roundtrips ... ok
test test_save_writes_file_once ... ok
test test_different_content_produces_different_hash ... ok
test test_get_after_duplicate_save_returns_original_content ... ok
test test_list_with_job_id_filter ... ok
test test_list_without_filter_returns_all ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace test suite: 147 tests passed, 0 failed.

## Format Gate

```
(no output — exit 0)
```

## Platform Cross-Check

```
CHECK 1 (mock-hardware Linux):     Finished `dev` profile [...] — PASS
CHECK 2 (mock-hardware Windows):   Finished `dev` profile [...] — PASS
CHECK 3 (real-hardware Linux):     Finished `dev` profile [...] — PASS
CHECK 4 (real-hardware Windows):   Finished `dev` profile [...] — PASS
```

All four cross-checks passed with zero errors.

## Project Gates

```
Gate 1 (config_reference):
test tests::config_reference_matches_defaults ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

No config fields were changed, so the config drift gate passed without modification.

## Public API Delta

```
+    pub async fn list(&self, job_id: Option<Uuid>) -> Result<Vec<ArtifactMeta>, AnvilError> {
```

One new public item: `pub async fn list` on `ArtifactStore` — matches the plan's Public API Surface table exactly. The private `map_row` helper is not exposed.

## Deviations from Plan

1. **`ArtifactMeta` does not derive `sqlx::FromRow`** — The plan assumed `ArtifactMeta` could be used directly with `query_as::<_, ArtifactMeta>`. However, `PathBuf` (the `file_path` field) is not a native sqlx SQLite type, and orphan rules prevent implementing `FromRow` for `ArtifactMeta` (defined in `anvilml-core`) from `anvilml-artifacts`. Resolved by using `sqlx::query()` + manual row mapping via a private `map_row` helper function.

2. **`uuid` moved to regular dependencies** — The plan assumed `uuid` was already available as a regular dependency. It was only in `[dev-dependencies]`, so it was added to `[dependencies]` for the `map_row` helper's UUID parsing.

3. **`ensure_artifacts_table()` called in `list()`** — The plan did not mention this, but `list()` must ensure the table exists because it can be called without any prior `save()`. Added `self.ensure_artifacts_table().await?` at the start of `list()`.

4. **Test data: three distinct PNG bytes** — The plan's `test_list_without_filter_returns_all` described saving three artifacts under two job IDs. Since `save()` is idempotent (same bytes → same hash → `INSERT OR IGNORE`), two of the saves used different PNG bytes (TEST_PNG, TEST_PNG_WHITE) and the third used a modified copy of TEST_PNG to guarantee unique hashes.

## Blockers

None.
