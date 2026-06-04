# Plan Report: P6-A4

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P6-A4                                         |
| Phase       | 006 — Model Registry                          |
| Description | anvilml-registry: ModelRegistry rescan (scan + bulk upsert) |
| Depends on  | P6-A3                                           |
| Project     | anvilml                                         |
| Planned at  | 2026-06-04T06:00:00Z                            |
| Attempt     | 1                                               |

## Objective

Add `async fn rescan` to `ModelRegistry` in `store.rs` that calls `scan_dirs` on the provided directory configurations, then upserts each discovered `ModelMeta` into SQLite via the existing `upsert` method, returning the total count of models upserted. The operation must be idempotent: running rescan twice over the same files must not create duplicates or remove pre-existing rows.

## Scope

### In Scope
- Add `async fn rescan(&self, dirs: &[ModelDirConfig]) -> Result<u32>` to `store.rs`
- The method calls `anvilml_registry::scanner::scan_dirs(dirs)` then iterates the result calling `self.upsert(&meta)` for each entry
- Returns the count of models processed (length of scanned results)
- No stale-model removal — only additive/upsert behavior
- Integration test file `crates/anvilml-registry/tests/rescan.rs` with two scenarios:
  1. First rescan on a tempdir with N model files returns count N and all N appear in the store
  2. Second rescan on the same tempdir returns count N and the store still has exactly N rows (idempotent)

### Out of Scope
- REST endpoint wiring (`POST /v1/models/rescan`) — that is P6-A7
- Startup scan at `main.rs` — that is P6-A5
- Stale model removal or cleanup logic
- Any changes to scanner.rs, lib.rs, db.rs, Cargo.toml, or any other crate

## Approach

### Step 1 — Add `rescan` method to `store.rs`

Add a new public async method on `impl ModelRegistry`:

```rust
/// Scan the configured model directories and upsert every discovered
/// model into the registry. Returns the total number of models processed.
///
/// This operation is idempotent: calling it multiple times over the same
/// files will not create duplicates because `upsert` uses `INSERT OR REPLACE`.
/// Stale rows (files deleted from disk) are NOT removed — manual cleanup
/// is required via a separate mechanism.
pub async fn rescan(&self, dirs: &[ModelDirConfig]) -> Result<u32, AnvilError> {
    let metas = scan_dirs(dirs).await;
    for meta in &metas {
        self.upsert(meta).await?;
    }
    Ok(metas.len() as u32)
}
```

The `scan_dirs` function is already re-exported from `lib.rs` (`pub use scanner::scan_dirs`), so the method can call it directly. No new dependencies are needed.

### Step 2 — Add integration test file `tests/rescan.rs`

Create a new test file following the existing pattern in `tests/store_get.rs` and `tests/store_list.rs`:

```rust
//! Integration tests for `ModelRegistry::rescan`.

use std::path::PathBuf;

use anvilml_core::config::ModelDirConfig;
use anvilml_core::{DType, ModelKind};
use sqlx::SqlitePool;

async fn open_pool(path: &std::path::Path) -> SqlitePool {
    anvilml_registry::db::open(path).await.unwrap()
}

/// First rescan on a tempdir with 2 model files should upsert both and return count 2.
#[tokio::test]
async fn test_rescan_adds_models() { ... }

/// Second rescan over the same tempdir keeps exactly N rows (idempotent).
#[tokio::test]
async fn test_rescan_idempotent() { ... }
```

**`test_rescan_adds_models`:**
1. Create a tempdir and write 2 model files (`.safetensors`) with distinct content.
2. Open a pool, create `ModelRegistry`, call `rescan(&[ModelDirConfig{path, kind: Some(Diffusion)}])`.
3. Assert returned count is `2`.
4. Call `list(None)` and assert length is `2`.

**`test_rescan_idempotent`:**
1. Same setup as above (tempdir with 2 files).
2. First `rescan` → count = 2, list length = 2.
3. Second `rescan` on the same config → count = 2, list length still = 2.
4. Verify model IDs are identical across both runs (same canonical paths → same SHA-256 → same id).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-registry/src/store.rs` | Add `rescan` method to `impl ModelRegistry` |
| Create | `crates/anvilml-registry/tests/rescan.rs` | Integration tests for rescan (adds, idempotent) |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `tests/rescan.rs` | `test_rescan_adds_models` | First rescan on tempdir with 2 files returns count 2 and all 2 appear in the store |
| `tests/rescan.rs` | `test_rescan_idempotent` | Second rescan on same files returns count 2, store still has exactly 2 rows, IDs match |

## CI Impact

No CI changes required. The task only adds a method and an integration test within the existing crate. The existing CI command `cargo test --workspace --features mock-hardware` will automatically discover and run the new test file. No new dev-dependencies are needed (all used crates — `sqlx`, `tempfile`, `tokio`, `anvilml-core` — are already present in `[dev-dependencies]`).

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| `scan_dirs` is re-exported from `lib.rs` — if the re-export path changes, the method call would break. | The re-export `pub use scanner::scan_dirs` is stable (established in P6-A1); verify it exists before writing. |
| `ModelDirConfig` import: needs to be available in `store.rs`. It lives in `anvilml_core::config`. | Add the import `use anvilml_core::config::ModelDirConfig;` at the top of `store.rs` alongside existing imports. |
| Test file not discovered by cargo test. | Follow the existing convention: place under `crates/anvilml-registry/tests/` as `rescan.rs`; Cargo auto-discovers `tests/*.rs` as integration tests. |
| Idempotency relies on `INSERT OR REPLACE` using the `id` column. | The `upsert` method already uses `INSERT OR REPLACE INTO models (id, ...)` with `id` as PRIMARY KEY — verified in P6-A2 implementation. No change needed. |

## Acceptance Criteria

- [ ] `store.rs` contains `pub async fn rescan(&self, dirs: &[ModelDirConfig]) -> Result<u32, AnvilError>` that calls `scan_dirs` then upserts each result
- [ ] `rescan` never removes stale rows (only additive/upsert behavior)
- [ ] Test file `tests/rescan.rs` exists with 2 test functions
- [ ] `cargo test -p anvilml-registry -- rescan` exits 0 (both tests pass)
- [ ] First rescan on tempdir adds N models, second rescan keeps exactly N (idempotent)
