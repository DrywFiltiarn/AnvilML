# Plan Report: P905-A4

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P905-A4                                           |
| Phase       | 905 — FP8 dtype + model registry metadata         |
| Description | anvilml-registry: remove stale model records on rescan |
| Depends on  | P905-A3                                           |
| Project     | anvilml                                           |
| Planned at  | 2026-06-12T12:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Extend `ModelRegistry::rescan` in `anvilml-registry` to detect and delete model records whose file paths no longer exist on disk. When model files are removed from scanned directories, a subsequent rescan should prune those stale database rows so the registry accurately reflects only models that currently exist.

## Scope

### In Scope
- Modify `anvilml-registry/src/store.rs`: change `rescan` return type from `Result<u32>` to `Result<(usize, usize)>` (upserted, removed); after upserting fresh models, query DB for all model IDs whose `path` starts with any scanned dir root; compute stale set (DB paths minus fresh paths); DELETE each stale row; add INFO log for scan complete with upserted and removed counts; add DEBUG log for each stale deletion.
- Modify `crates/anvilml-server/src/handlers/models.rs`: update `rescan_models` handler to destructure the `(upserted, removed)` tuple; pass `removed` into the log message (e.g. `removed_stale = removed`).
- Modify `backend/src/main.rs`: update the initial background rescan spawn to destructure the tuple and log both counts.
- Add integration test `crates/anvilml-registry/tests/rescan_stale.rs`: scan 2 files, delete 1, rescan, assert DB has 1 row and removed == 1.
- Bump `anvilml-registry` patch version from `0.1.2` to `0.1.3` in `Cargo.toml`.

### Out of Scope
- No changes to the HTTP response body of `POST /v1/models/rescan` (it returns `RescanResponse` with only `status: "rescan_started"`; stale count is logged but not returned).
- No new API endpoint for listing stale models.
- No changes to the `models` table schema or migrations.
- No changes to `anvilml-server` route wiring (no new routes).
- No changes to OpenAPI schema (handler signatures unchanged at the HTTP level).

## Approach

1. **Extend `rescan` in `store.rs`** (lines 160–173):
   - Change signature from `pub async fn rescan(&self, dirs: &[ModelDirConfig]) -> Result<u32, AnvilError>` to `pub async fn rescan(&self, dirs: &[ModelDirConfig]) -> Result<(usize, usize), AnvilError>`.
   - Collect fresh model IDs into a `HashSet<String>` during the upsert loop.
   - Query the DB for all model rows whose `path` column starts with any scanned directory root:
     ```sql
     SELECT id, path FROM models WHERE path LIKE ? || '%'
     ```
     (One query per dir root, or a single query with `OR` chains.)
   - Compute `stale_ids` = DB rows whose `path` is not in the fresh set.
   - DELETE each stale row:
     ```sql
     DELETE FROM models WHERE id = ?
     ```
   - Return `(metas.len(), stale_ids.len())`.
   - Add `tracing::info!(models_scanned = upserted, removed_stale = removed, "background rescan complete")` to match the log format used by callers.
   - Add `tracing::debug!(path = %path, "rescan: removed stale model")` for each deleted row.

2. **Update `handlers/models.rs`** (lines 107–124):
   - In `rescan_models`, change the tokio spawn closure to destructure `Ok((upserted, removed))` and log:
     ```rust
     tracing::info!(models_scanned = upserted, removed_stale = removed, "background rescan complete")
     ```

3. **Update `backend/src/main.rs`** (lines 231–235):
   - Change `Ok(count)` to `Ok((upserted, removed))` and log:
     ```rust
     tracing::info!(models_scanned = upserted, removed_stale = removed, "initial model scan complete")
     ```

4. **Add integration test** `crates/anvilml-registry/tests/rescan_stale.rs`:
   - Follow the pattern of `tests/rescan.rs` (tempfile for DB, tempdir for models).
   - Write 2 `.safetensors` files in a model directory.
   - First rescan → assert count is 2, DB has 2 rows.
   - Delete 1 file from disk.
   - Second rescan → assert returned tuple is `(1, 1)`, DB has 1 row.

5. **Bump version** in `crates/anvilml-registry/Cargo.toml`: `0.1.2` → `0.1.3`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-registry/src/store.rs` | Extend `rescan` to detect and delete stale model records; change return type to `Result<(usize,usize)>`. |
| Modify | `crates/anvilml-server/src/handlers/models.rs` | Update `rescan_models` caller to destructure `(upserted, removed)` tuple. |
| Modify | `backend/src/main.rs` | Update initial rescan caller to destructure `(upserted, removed)` tuple. |
| Create   | `crates/anvilml-registry/tests/rescan_stale.rs` | Integration test: scan 2 files, delete 1, rescan, assert 1 stale removed. |
| Modify | `crates/anvilml-registry/Cargo.toml` | Bump patch version `0.1.2 → 0.1.3`. |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-registry/tests/rescan_stale.rs` | `test_rescan_removes_stale_models` | After scanning 2 models, deleting 1 file, and rescanning, DB has 1 row and removed count == 1. |
| *(existing)* | `test_rescan_adds_models` (in `tests/rescan.rs`) | Still passes — no regression on basic add behavior. |
| *(existing)* | `test_rescan_idempotent` (in `tests/rescan.rs`) | Still passes — rescanning same files returns `(N, 0)` removed. |

## CI Impact

No CI workflow changes required. The task only modifies source code and adds a new integration test under the existing `anvilml-registry` crate, which is already covered by `cargo test --workspace --features mock-hardware`. No new feature flags are introduced. The OpenAPI drift gate (§8 of ENVIRONMENT.md) is not triggered because no handler signature, response type, or `#[utoipa::path]` annotation changes.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| SQL `LIKE` query with `path LIKE ? || '%'` may match unintended paths (e.g. `/models/diffusion` matching `/models/diffusion_extra`) | Low | Low | Use exact prefix matching: `path LIKE ? || '%'` is correct since scanned dir roots are explicit config paths; a subdirectory path `/models/diffusion` will not match `/models/diffusion_extra` because the `%` suffix only matches content *after* the root. If paranoid, append a `/` to the root: `path LIKE ? || '/%' OR path = ?`. |
| Deleting stale rows changes semantics of existing tests that expect `rescan` to never delete | Low | Low | Existing tests in `tests/rescan.rs` only test idempotent rescans with no deletions — they will return `(N, 0)` and pass unchanged. |
| Concurrent rescan calls could race on stale detection | Low | Medium | Rescan already runs in a background tokio task; the existing handler spawns a new task per call. The `INSERT OR REPLACE` + `DELETE` pattern is atomic per-statement. For safety, wrap the stale-detection block in a single transaction. |
| `path` column UNIQUE constraint prevents duplicate inserts but does not affect deletes | None | None | No action needed — the constraint is already in place and does not interfere with DELETE. |

## Acceptance Criteria

- [ ] `rescan` return type is `Result<(usize, usize), AnvilError>` in `store.rs`
- [ ] After upserting fresh models, stale paths are detected via DB query and deleted
- [ ] `handlers/models.rs` and `main.rs` callers destructure the tuple and log both counts
- [ ] `crates/anvilml-registry/tests/rescan_stale.rs` exists with test: scan 2 files, delete 1, rescan, assert removed == 1 and DB has 1 row
- [ ] `cargo test -p anvilml-registry` exits 0
- [ ] `anvilml-registry` patch version bumped to `0.1.3`
- [ ] Mandatory INFO log point: `models_scanned` field present at scan completion (§9 of ENVIRONMENT.md)
- [ ] Mandatory DEBUG log point: each file examined logged (`path=` field) — already present in scanner (§11.5 of FORGE_AGENT_RULES.md), no change needed
