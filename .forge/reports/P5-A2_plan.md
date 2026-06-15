# Plan Report: P5-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P5-A2                                       |
| Phase       | 005 — SQLite Persistence                    |
| Description | SeedLoader SHA256-gated SQL seed runner      |
| Depends on  | P5-A1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-15T14:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Implement a SHA256-gated SQL seed loader in `crates/anvilml-registry/src/seed_loader.rs` that discovers `.sql` files in a configurable directory (`database/seeds/`), computes SHA256 of each file's content, compares against the `seed_history` table, and either skips (up-to-date) or executes + records (changed or new). Create the initial seed file `database/seeds/devices.sql` with `INSERT OR IGNORE INTO device_capabilities` rows for every entry in `docs/SUPPORTED_DEVICES_DB.md`. The observable outcome is that `cargo test -p anvilml-registry -- seed` exits 0 on first run (applies seeds) and exits 0 on second run (skips all seeds via SHA256 match).

## Scope

### In Scope
- Create `crates/anvilml-registry/src/seed_loader.rs` with `pub async fn run(pool: &SqlitePool, seeds_path: &Path) -> Result<(), AnvilError>`
- Create `crates/anvilml-registry/tests/seed_loader_tests.rs` with tests
- Create `database/seeds/devices.sql` with `INSERT OR IGNORE INTO device_capabilities` rows for all entries in `docs/SUPPORTED_DEVICES_DB.md`
- Update `crates/anvilml-registry/Cargo.toml` to add `sha2 = "0.10"` dependency and bump patch version (0.1.1 → 0.1.2)
- Update `crates/anvilml-registry/src/lib.rs` to `pub mod seed_loader;` and `pub use seed_loader::run;`

### Out of Scope
- Integration wiring of `seed_loader::run()` into `db.rs` or `main.rs` (handled by P5-B1 or a future task)
- Seed file management beyond SHA256 gating (no rollback, no versioning of seeds)
- Support for non-`.sql` seed files
- Seed directory traversal beyond flat listing (no recursive scan)

## Existing Codebase Assessment

The `anvilml-registry` crate currently has `lib.rs` (12 lines, re-exports only), `db.rs` (168 lines, open/open_in_memory/migrations/ghost-reset), and one test file `tests/db_tests.rs` (264 lines). The `seed_history` table already exists in migration `001_initial.sql` with columns `(file TEXT PRIMARY KEY, sha256 TEXT NOT NULL, applied_at TEXT NOT NULL)`. The `device_capabilities` table also exists with the schema from `SUPPORTED_DEVICES_DB.md §Migration DDL`. The `sha2` crate (0.10.9) is already a transitive dependency in `Cargo.lock` and will be added as a direct dependency. Error handling follows the `AnvilError` enum from `anvilml-core`, with `Io` for file I/O failures and `Db` for SQL failures. Logging uses `tracing::info!` with structured fields. The test style uses `#[tokio::test]` async tests, `tempfile::tempdir()` for unique paths, and `sqlx::query_scalar` for assertions.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | sha2    | 0.10.9          | Cargo.lock fallback (MCP rust-docs unavailable for this task) | n/a |

The `sha2` crate 0.10.9 is already present in `Cargo.lock` as a transitive dependency (via `digest 0.10.7`). The API shape (`Sha256`, `Digest` trait, `update()`, `finalize()`) is confirmed stable across 0.10.x. If the MCP rust-docs tool is available at ACT time, the ACT agent must confirm the version matches 0.10.9 or higher.

## Approach

1. **Add `sha2` dependency to `anvilml-registry/Cargo.toml`.** Add `sha2 = "0.10"` under `[dependencies]`. This is a minimal, zero-feature dependency that provides `Sha256` and the `Digest` trait. Rationale: sha2 0.10.x is already a transitive dependency in the lockfile, so adding it directly avoids introducing a second version of sha2.

2. **Create `crates/anvilml-registry/src/seed_loader.rs`.** Implement `pub async fn run(pool: &SqlitePool, seeds_path: &Path) -> Result<(), AnvilError>`:
   - Read directory entries from `seeds_path` using `std::fs::read_dir`.
   - Filter for `.sql` files only.
   - For each `.sql` file:
     a. Read the entire file content via `std::fs::read` (seed files are small, no streaming needed).
     b. Compute SHA256 hex digest using `sha2::Sha256` and the `Digest` trait: `Sha256::digest(&content)` → `format!("{:x}", result)`.
     c. Query `SELECT sha256 FROM seed_history WHERE file = ?` with the file's canonical path.
     d. If a row exists and the SHA256 matches → log `tracing::info!(file = %file_name, status = "up-to-date", "seed skipped")` and continue to next file.
     e. If no row exists or SHA256 differs → execute the SQL via `sqlx::query_file` or `sqlx::query` with the loaded text, then insert `INSERT OR REPLACE INTO seed_history (file, sha256, applied_at) VALUES (?, ?, ?)` with the file path, SHA256 hex, and `chrono::Utc::now()` as `applied_at`. Log `tracing::info!(file = %file_name, sha256 = %sha256_hex, "seed applied")`.
   - Return `Ok(())` after processing all files.
   - Apply `#[tracing::instrument]` to the function per ENVIRONMENT.md §9 mandatory log points and §11.5.
   - Add `///` doc comment on the public function describing arguments, return value, and error variants.

3. **Update `crates/anvilml-registry/src/lib.rs`.** Add `pub mod seed_loader;` and `pub use seed_loader::run;` to expose the new module and function. This keeps lib.rs under 80 lines.

4. **Create `database/seeds/devices.sql`.** Generate `INSERT OR IGNORE INTO device_capabilities` rows for every entry in `docs/SUPPORTED_DEVICES_DB.md`. Convert each Markdown table row into a SQL INSERT statement:
   - `vendor_id` and `device_id` from the hex values (strip `0x` prefix, parse as decimal INTEGER).
   - `name` from the `name` column.
   - `arch` from the `arch` column.
   - Capability booleans (`fp32`, `fp16`, `bf16`, `fp8`, `fp4`, `flash_attention`) mapped: `Y → 1`, `N → 0`.
   - Use `INSERT OR IGNORE` to make the seed idempotent (primary key is `(vendor_id, device_id)`).
   - Include a SQL comment header referencing `docs/SUPPORTED_DEVICES_DB.md` as the source.

5. **Create `crates/anvilml-registry/tests/seed_loader_tests.rs`.** Write two tests:
   - `test_seed_loader_applies_new_seed`: Create a temp directory with a `.sql` seed file, call `run()`, verify the seed was applied (row in `seed_history`, rows in `device_capabilities`).
   - `test_seed_loader_skips_up_to_date`: Run `run()` twice on the same temp directory; second run must skip (no additional `seed_history` rows inserted).
   - Each test uses its own `open_in_memory()` pool for isolation.

6. **Bump `anvilml-registry` patch version** from `0.1.1` to `0.1.2` in `Cargo.toml` per §12 of ENVIRONMENT.md and §14 of FORGE_AGENT_RULES.

## Public API Surface

| Item | Module Path | Signature |
|------|-------------|-----------|
| `run` | `anvilml_registry::seed_loader` | `pub async fn run(pool: &SqlitePool, seeds_path: &Path) -> Result<(), AnvilError>` |

No new structs, enums, or traits. The function is the sole public API.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-registry/src/seed_loader.rs` | SeedLoader: SHA256-gated SQL seed runner |
| CREATE | `crates/anvilml-registry/tests/seed_loader_tests.rs` | Tests for seed_loader: apply and skip behavior |
| CREATE | `database/seeds/devices.sql` | Device capability seed data from SUPPORTED_DEVICES_DB.md |
| MODIFY | `crates/anvilml-registry/src/lib.rs` | Add `pub mod seed_loader;` and `pub use seed_loader::run;` |
| MODIFY | `crates/anvilml-registry/Cargo.toml` | Add `sha2 = "0.10"` dependency; bump version 0.1.1 → 0.1.2 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-registry/tests/seed_loader_tests.rs` | `test_seed_loader_applies_new_seed` | First run applies seed: inserts into `seed_history` and executes SQL into `device_capabilities` | Temp dir with one `.sql` seed file containing INSERT statements | In-memory pool, temp dir path | `seed_history` has 1 row; `device_capabilities` has rows from the seed SQL | `cargo test -p anvilml-registry -- seed_loader_applies_new_seed` exits 0 |
| `crates/anvilml-registry/tests/seed_loader_tests.rs` | `test_seed_loader_skips_up_to_date` | Second run skips: `seed_history` unchanged, no SQL re-execution | Same temp dir from first test (seed already applied) | In-memory pool, same temp dir path | `seed_history` still has 1 row (no duplicate); `device_capabilities` unchanged | `cargo test -p anvilml-registry -- seed_loader_skips_up_to_date` exits 0 |

## CI Impact

No CI changes required. The new test file lives in `crates/anvilml-registry/tests/` which is already picked up by `cargo test --workspace --features mock-hardware`. The new `sha2` dependency is a direct dependency with no feature flags, so it does not affect the CI job matrix.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. File system operations (`std::fs::read_dir`, `std::fs::read`) use standard library paths that work cross-platform. The SHA256 computation is platform-neutral. SQLite operations via `sqlx` are platform-neutral. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `sqlx::query_file` may not exist or may have a different API in sqlx 0.9.0. The file path must be resolved relative to CARGO_MANIFEST_DIR or the working directory. | Medium | High | Verify `sqlx::query_file` exists in sqlx 0.9.0 at ACT time via Cargo.lock. If unavailable, use `std::fs::read_to_string` to load SQL text and `sqlx::query` with the text as a string parameter. |
| The `sha2` crate API (`Sha256::digest`) may differ between 0.10.x and 0.11.x. The plan uses 0.10.x which is in Cargo.lock as transitive, but adding as direct dep could cause version resolution conflicts. | Low | Medium | Use `sha2 = "0.10"` (compatible range) — if the lockfile already has 0.10.9, cargo will reuse it. If a conflict arises, the ACT agent resolves by pinning to the exact version in Cargo.lock (0.10.9). |
| The `devices.sql` file is large (~360 INSERT statements for NVIDIA + ~60 for AMD). A single SQL file with many INSERTs may slow down seed execution. | Low | Low | `INSERT OR IGNORE` is efficient for SQLite with a UNIQUE index on `(vendor_id, device_id)`. If performance is an issue at ACT time, batch into groups of 50, but this is unlikely needed. |
| `seed_history.applied_at` uses `chrono::Utc::now()` which requires the `chrono` crate. `anvilml-registry` does not currently depend on `chrono`. | Medium | Medium | Add `chrono = { workspace = true }` to `anvilml-registry/Cargo.toml` (the workspace already has `chrono = { version = "0.4.45", features = ["serde"] }`). Alternatively, use `std::time::SystemTime` and format as ISO 8601 string without chrono. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-registry -- seed_loader_applies_new_seed` exits 0
- [ ] `cargo test -p anvilml-registry -- seed_loader_skips_up_to_date` exits 0
- [ ] `cargo test -p anvilml-registry -- seed` exits 0 (both tests together)
