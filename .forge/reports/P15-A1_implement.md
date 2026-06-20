# Implementation Report: P15-A1

| Field         | Value                                                |
|---------------|------------------------------------------------------|
| Task ID       | P15-A1                                               |
| Phase         | 015 — Artifact Storage                               |
| Description   | anvilml-server: artifact/store.rs content-addressed PNG storage |
| Implemented   | 2026-06-20T14:15:00Z                                 |
| Status        | COMPLETE                                             |

## Summary

Implemented content-addressed PNG artifact storage in `ArtifactStore` (`crates/anvilml-server/src/artifact/store.rs`). The store persists PNG images to disk by their SHA-256 content hash and records metadata in the `artifacts` SQLite table. Added `width` and `height` fields to `ArtifactMeta` in `anvilml-core`. Created migration 003 to add the `width` and `height` columns to the `artifacts` table. All 5 integration tests pass, all workspace tests pass (223 total), and all project gates pass.

## Resolved Dependencies

| Type   | Name       | Version resolved | Source          |
|--------|------------|------------------|-----------------|
| crate  | sha2       | 0.10.9           | Cargo.lock      |
| crate  | serial_test| 3.5.0            | Cargo.lock      |

`sha2 = "0.10"` was already declared in `anvilml-registry/Cargo.toml` — Cargo.lock confirms version 0.10.9. The API shape used is `sha2::{Sha256, Digest}` — `Sha256::new()`, `.update(&data)`, `.finalize()` formatted via `format!("{:x}", result)`.
`serial_test = "3.5"` was already in Cargo.lock at version 3.5.0.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `crates/anvilml-core/src/types/artifact.rs` | Added `pub width: u32` and `pub height: u32` fields to `ArtifactMeta` after `hash` |
| MODIFY | `crates/anvilml-core/tests/artifact_tests.rs` | Added `width`/`height` to test struct initializers and assertions |
| CREATE | `crates/anvilml-server/src/artifact/mod.rs` | Module declaration + re-export: `pub mod store; pub use store::ArtifactStore;` |
| CREATE | `crates/anvilml-server/src/artifact/store.rs` | `ArtifactStore` struct with `new`, `save`, `get`, `list` methods |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Added `pub mod artifact;` |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Added `sha2 = "0.10"` dependency, `serial_test = "3.5"` dev-dep, bumped version 0.1.22 → 0.1.23 |
| CREATE | `crates/anvilml-server/tests/artifact_store_tests.rs` | 5 integration tests for `ArtifactStore` |
| CREATE | `database/migrations/003_add_artifact_dimensions.sql` | Added `width` and `height` INTEGER columns to `artifacts` table |
| MODIFY | `docs/TESTS.md` | Added 5 test entries for new artifact store tests |

## Commit Log

```
 .forge/reports/P15-A1_plan.md                      | 157 +++++++++++
 .forge/state/CURRENT_TASK.md                       |   6 +-
 .forge/state/state.json                            |  13 +-
 Cargo.lock                                         |   4 +-
 crates/anvilml-core/src/types/artifact.rs          |   4 +
 crates/anvilml-core/tests/artifact_tests.rs        |  14 +
 crates/anvilml-server/Cargo.toml                   |   4 +-
 crates/anvilml-server/src/artifact/mod.rs          |   9 +
 crates/anvilml-server/src/artifact/store.rs        | 288 +++++++++++++++++++++
 crates/anvilml-server/src/lib.rs                   |   1 +
 crates/anvilml-server/tests/artifact_store_tests.rs| 207 +++++++++++++++
 database/migrations/003_add_artifact_dimensions.sql|  11 +
 docs/TESTS.md                                      |  45 ++++
 13 files changed, 752 insertions(+), 11 deletions(-)
```

## Test Results

```
     Running tests/artifact_store_tests.rs (target/debug/deps/artifact_store_tests-4573af4a8fba7178)

running 5 tests
test test_get_returns_none_for_unknown_hash ... ok
test test_hash_is_deterministic ... ok
test test_list_returns_saved_artifact ... ok
test test_save_and_get_roundtrip ... ok
test test_save_is_idempotent ... ok

test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.06s
```

Full workspace test suite: 223 tests passed, 0 failed. All existing tests continue to pass.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no drift after pass 3)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware → Finished

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu → Finished

# 3. Real-hardware Linux
cargo check --bin anvilml → Finished

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu → Finished
```

All four cross-checks passed with zero errors.

## Project Gates

```
Gate 1 — Config Surface Sync:
  cargo test -p anvilml --features mock-hardware -- config_reference
  → test config_reference ... ok
  → test result: ok. 1 passed; 0 failed

Gate 2 — OpenAPI Drift:
  cargo run -p anvilml-openapi && git diff --exit-code api/openapi.json
  → EXIT: 0 (no drift)
```

## Public API Delta

```
crates/anvilml-core/src/types/artifact.rs:
  +pub width: u32,
  +pub height: u32,

crates/anvilml-server/src/artifact/mod.rs:
  +pub mod store;
  +pub use store::ArtifactStore;

crates/anvilml-server/src/artifact/store.rs:
  +pub struct ArtifactStore { ... }
  +pub async fn new(dir: PathBuf, db: SqlitePool) -> Self
  +pub async fn save(&self, job_id: Uuid, image_bytes: &[u8]) -> Result<ArtifactMeta>
  +pub async fn get(&self, hash: &str) -> Result<Option<PathBuf>>
  +pub async fn list(&self, job_id: Option<Uuid>) -> Result<Vec<ArtifactMeta>>

crates/anvilml-server/src/lib.rs:
  +pub mod artifact;
```

## Deviations from Plan

- **Added migration 003** (`database/migrations/003_add_artifact_dimensions.sql`): The plan stated "No new migration is needed" because the `artifacts` table already exists. However, the `artifacts` table in migration `001_initial.sql` does NOT have `width` and `height` columns. The `INSERT OR IGNORE` statement in `ArtifactStore::save` references these columns, so without a migration the code would fail at runtime with a "no such column" error. Added migration 003 to add `width INTEGER` and `height INTEGER` columns to the `artifacts` table.
- **Fixed `sqlx::types::Null` → `Option::<String>::None`**: sqlx 0.9.0 does not have `sqlx::types::Null`. Used `bind::<Option<String>>(None)` to bind NULL values for the width and height columns.
- **Fixed UUID column decode**: The `job_id` column in the `artifacts` table is `TEXT` (UUID hex string), not `UUID` type. Used `row.get::<String, _>("job_id")` and parsed to `Uuid`.
- **Fixed `id` column decode**: The `id` column is `INTEGER PRIMARY KEY AUTOINCREMENT`, not `TEXT`. Read as `i64` and formatted to `String` to match `ArtifactMeta::id`.
- **Added `serial_test` annotation to tests**: All 5 tests are annotated with `#[serial]` because they share filesystem state (temp directory) and database state (in-memory pool with `open_in_memory()`).
- **Fixed existing `artifact_tests.rs`**: Added `width` and `height` fields to the two `ArtifactMeta` struct initializers in the existing test file that were broken by the struct modification.
- **Fixed clippy `redundant_closure`**: Used `.map_err(AnvilError::Io)` instead of `.map_err(|e| AnvilError::Io(e))`.

## Blockers

None.
