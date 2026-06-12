# Plan Report: P906-A5

| Field       | Value                                               |
|-------------|-----------------------------------------------------|
| Task ID     | P906-A5                                             |
| Phase       | 906 — OpenAPI Spec Correctness Retrofit             |
| Description | anvilml-registry: fix stale-model LIKE query fails on Windows paths |
| Depends on  | P905-A6, P905-A7                                    |
| Project     | anvilml                                             |
| Planned at  | 2026-06-12T18:35:00Z                                |
| Attempt     | 1                                                   |

## Objective

Fix the stale-model removal logic in `anvilml-registry` so that the `rescan` method
correctly detects and removes stale model rows on Windows, where file paths use
backslash separators instead of forward slashes. The root cause is a SQL LIKE query
with a hardcoded `/` separator that never matches Windows backslash paths.

## Scope

### In Scope
- Add a `norm_path` helper function that converts backslashes to forward slashes
- Normalise path strings in `upsert` before DB write
- Normalise path strings in `rescan` for both fresh-path collection and DB query
- Replace the two-pass `LIKE`+`exact` SQL in `rescan` with a single `SELECT id, path FROM models`
  followed by Rust-side filtering using normalised path prefix comparison
- Bump `anvilml-registry` patch version from `0.1.4` to `0.1.5` in `Cargo.toml`

### Out of Scope
- No changes to test files (the existing `rescan_stale.rs` test is correct)
- No changes to `scanner.rs` (scanner already uses canonical paths; normalisation
  happens at the store boundary)
- No changes to `ModelMeta` struct or any other crate
- No OpenAPI spec regeneration
- No logging changes (this is a correctness fix, not an observability change)

## Approach

1. **Add `norm_path` helper** in `store.rs`:
   ```rust
   /// Normalise a path string: replace all backslashes with forward slashes.
   fn norm_path(p: &str) -> String {
       p.replace('\\', "/")
   }
   ```

2. **Normalise path in `upsert`** (line 54):
   Change from:
   ```rust
   .bind(meta.path.to_string_lossy().to_string())
   ```
   To:
   ```rust
   .bind(norm_path(&meta.path.to_string_lossy()))
   ```

3. **Normalise paths in `rescan`** — three locations:
   - **Fresh paths collection** (line 181):
     ```rust
     let fresh_paths: HashSet<String> = metas
         .iter()
         .map(|m| norm_path(&m.path.to_string_lossy()))
         .collect();
     ```
   - **Replace the two-pass SQL** (lines 185–207) with a single query + Rust filter:
     ```rust
     // Fetch all DB rows for stale detection.
     let all_rows: Vec<(String, String)> =
         sqlx::query_as("SELECT id, path FROM models")
             .fetch_all(&self.pool)
             .await
             .map_err(sqlx_error)?;

     // Normalised dir prefixes for filtering.
     let dir_prefixes: Vec<String> = dirs
         .iter()
         .map(|d| norm_path(&d.path.to_string_lossy()))
         .collect();

     // Filter: row path starts with any normalised dir prefix and is not fresh.
     let stale_ids: Vec<String> = all_rows
         .into_iter()
         .filter(|(_, path)| {
             let path_norm = norm_path(path);
             dir_prefixes.iter().any(|prefix| path_norm.starts_with(prefix))
                 && !fresh_paths.contains(&path_norm)
         })
         .map(|(id, _)| id)
         .collect();
     ```

4. **Bump `anvilml-registry` patch version** in `Cargo.toml`:
   `0.1.4` → `0.1.5`

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-registry/src/store.rs` | Add `norm_path` helper; normalise paths in `upsert` and `rescan`; replace two-pass SQL with single query + Rust filter |
| Modify | `crates/anvilml-registry/Cargo.toml` | Bump patch version `0.1.4` → `0.1.5` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-registry/tests/rescan_stale.rs` | `test_rescan_removes_stale_models` | Rescan with one deleted file removes exactly 1 stale model (`removed == 1`) |

No new test files are added. The existing test already asserts the correct behaviour
(`removed == 1`) — the fix makes production code match the test's expectation on all
platforms.

## CI Impact

`cargo test -p anvilml-registry` must exit 0, and `cargo test --workspace --features mock-hardware`
must also exit 0. The change is confined to one crate (`anvilml-registry`) and does not
affect any other crate's public API, so no OpenAPI regeneration or config drift gate is
required.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Existing DB rows written with backslash paths before this fix will not match new forward-slash entries during stale detection | Medium | Low (ephemeral test DBs; production users trigger a rescan after upgrade) | Documented in TASKS_PHASE906.md — acceptable for the retrofit since DB is ephemeral in tests |
| Normalised paths differ between scanner-generated `fresh_paths` and DB-stored paths if one side is missed | Low | High (stale detection breaks entirely) | Apply `norm_path` consistently at all four points: `upsert` bind, fresh path collection, stale row path comparison, dir prefix computation |
| `norm_path` allocates a new `String` on every call | Low | Negligible (rescan is infrequent, not a hot path) | Acceptable; no performance concern for model registry operations |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-registry` exits 0
- [ ] `test_rescan_removes_stale_models` passes (asserts `removed == 1`)
- [ ] `cargo test --workspace --features mock-hardware` exits 0
- [ ] `anvilml-registry` patch version bumped to `0.1.5` in `Cargo.toml`
- [ ] No new warnings from `cargo clippy -p anvilml-registry -- -D warnings`
