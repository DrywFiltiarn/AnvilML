# Implementation Report: P6-B1

| Field         | Value                                             |
|---------------|---------------------------------------------------|
| Task ID       | P6-B1                                             |
| Phase         | 006 — Model Registry & Artifacts                  |
| Description   | anvilml-artifacts: ArtifactStore::save content-addressed write |
| Implemented   | 2026-06-30T00:00:00Z                              |
| Status        | COMPLETE                                          |

## Summary

Created `crates/anvilml-artifacts/src/store.rs` implementing `ArtifactStore::save()` — a content-addressed PNG write that computes the SHA256 hex digest of PNG bytes as the content hash, writes the file to `{artifact_dir}/{hash}.png` only if not already present (idempotent duplicate save), and persists the artifact metadata row to an SQLite database. Updated `lib.rs` to declare the module and re-export `ArtifactStore`. Created 3 integration tests in `tests/store_tests.rs` using tempdir + in-memory SQLite pools. All 136 workspace tests pass.

## Resolved Dependencies

| Type   | Name     | Version resolved | Source         |
|--------|----------|------------------|----------------|
| crate  | sha2     | 0.11.0           | rust-docs MCP  |
| crate  | tempfile | 3.27.0           | rust-docs MCP  |

Note: `sha2` 0.11.0 matches the version already used in `anvilml-registry/Cargo.toml`. The `tempfile` crate latest is 3.27.0; the project uses 3.26 in `anvilml-registry` dev-deps. The plan specified 3.26 — we use 3.26 to match the project convention. `sqlx` (0.9.0), `tokio` (1.47.0), `chrono` (0.4), and `tracing` (0.1) are declared in the crate's own `Cargo.toml` with the same versions as `anvilml-registry`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-artifacts/src/store.rs` | `ArtifactStore` struct with `new()` and `save()` methods |
| MODIFY | `crates/anvilml-artifacts/src/lib.rs` | Added `pub mod store;` and `pub use store::ArtifactStore;` |
| MODIFY | `crates/anvilml-artifacts/Cargo.toml` | Added dependencies: `sha2`, `sqlx`, `chrono`, `tracing`, `tokio` (dev), `tempfile` (dev), `uuid` (dev); bumped version 0.1.0 → 0.1.1 |
| CREATE | `crates/anvilml-artifacts/tests/store_tests.rs` | 3 integration tests for `save()` |
| CREATE | `crates/anvilml-artifacts/tests/fixtures/test_64x64_black.png` | 64×64 black PNG fixture (225 bytes) |
| CREATE | `crates/anvilml-artifacts/tests/fixtures/test_64x64_white.png` | 64×64 white PNG fixture (203 bytes) |
| MODIFY | `docs/TESTS.md` | Added 3 entries for new tests |

## Commit Log

```
 .forge/reports/P6-B1_plan.md                       | 202 ++++++++++++++++++
 .forge/state/CURRENT_TASK.md                       |   6 +-
 .forge/state/state.json                            |  13 +-
 Cargo.lock                                         |   9 +-
 crates/anvilml-artifacts/Cargo.toml                |  11 +-
 crates/anvilml-artifacts/src/lib.rs                |   8 +
 crates/anvilml-artifacts/src/store.rs              | 185 ++++++++++++++++
 .../tests/fixtures/test_64x64_black.png            | Bin 0 -> 225 bytes
 .../tests/fixtures/test_64x64_white.png            | Bin 0 -> 203 bytes
 crates/anvilml-artifacts/tests/store_tests.rs      | 233 +++++++++++++++++++++
 docs/TESTS.md                                      |  36 ++++
 11 files changed, 692 insertions(+), 11 deletions(-)
```

## Test Results

```
running 3 tests
test test_save_writes_file_once ... ok
test test_different_content_produces_different_hash ... ok
test test_duplicate_save_does_not_duplicate_or_error ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace test suite: 136 tests passed, 0 failed.

## Format Gate

```
# cargo fmt --all -- --check
# (exited 0 — no formatting drift)
```

## Platform Cross-Check

All four cross-checks passed:
1. `cargo check --workspace --features mock-hardware` — Finished successfully
2. `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` — Finished successfully
3. `cargo check --bin anvilml` — Finished successfully
4. `cargo check --bin anvilml --target x86_64-pc-windows-gnu` — Finished successfully

## Project Gates

Not applicable — this task does not modify `ServerConfig`, handler function signatures, `#[utoipa::path]` annotations, `AppState` fields, node types, or arch module functions. No gates are triggered.

## Public API Delta

```
+pub mod store;
+pub use store::ArtifactStore;
```

New public items:
- `pub mod store` — in `crates/anvilml-artifacts/src/lib.rs`
- `pub use store::ArtifactStore` — re-export in `crates/anvilml-artifacts/src/lib.rs`

These match the plan's `## Public API Surface` table exactly:
- struct: `pub struct ArtifactStore { artifact_dir: PathBuf, pool: SqlitePool }` (in store.rs)
- fn: `pub fn new(artifact_dir: PathBuf, pool: SqlitePool) -> Self` (in store.rs)
- fn: `pub async fn save(&self, png_bytes: &[u8], meta: &ArtifactMeta) -> Result<String, AnvilError>` (in store.rs)

## Deviations from Plan

1. **SHA256 hex formatting**: The plan specified `format!("{:x}", hasher.finalize())` but `sha2::Sha256::finalize()` returns a `GenericArray<u8, N>` which does not implement `LowerHex`. Fixed by iterating over bytes: `hasher.finalize().iter().map(|b| format!("{:02x}", b)).collect()`. This produces the same hex string output.

2. **INSERT vs INSERT OR IGNORE**: The plan specified a plain `INSERT` but the `artifacts` table has a `PRIMARY KEY` on `hash`. A duplicate save would violate the UNIQUE constraint. Changed to `INSERT OR IGNORE` so duplicate saves are a no-op at the DB level, matching the idempotent file-write behavior.

3. **Cargo.toml version**: The plan specified `version.workspace = true` with bump 0.1.0 → 0.1.1. Since the workspace version is read-only, changed to explicit `version = "0.1.1"` to achieve the same bump.

## Blockers

None.
