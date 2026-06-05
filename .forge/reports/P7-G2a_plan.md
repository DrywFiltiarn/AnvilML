# Plan Report: P7-G2a

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P7-G2a                                            |
| Phase       | 007 — WebSocket Event Stream                      |
| Description | anvilml-registry: seed_loader — tracking table bootstrap + SHA256 comparison |
| Depends on  | P7-G1                                             |
| Project     | anvilml                                           |
| Planned at  | 2026-06-05T15:03:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create `crates/anvilml-registry/src/seed_loader.rs` implementing the generic SQL seed loader infrastructure. On every run it bootstraps a `seed_history` tracking table, enumerates `.sql` files in a seeds directory, parses header directives (`-- anvil:seed_table <name>` and `-- anvil:seed_strategy <replace_all|merge>`), computes SHA256 of file contents, compares against the stored hash to skip unchanged files, and upserts the tracking row. The actual DELETE/INSERT execution is stubbed as a no-op (to be completed in P7-G2b).

## Scope

### In Scope
- Create `crates/anvilml-registry/src/seed_loader.rs` with:
  - `run(pool: &SqlitePool, seeds_dir: &Path) -> Result<(), AnvilError>` public async function
  - Bootstrap `CREATE TABLE IF NOT EXISTS seed_history(filename TEXT PRIMARY KEY, sha256 TEXT NOT NULL, applied_at INTEGER NOT NULL)` on every run
  - Enumerate `.sql` files in `seeds_dir` in sorted filename order
  - Parse header directives from first non-empty lines starting with `-- anvil:`
  - Reject files missing the `-- anvil:seed_table` directive (fatal error)
  - Compute SHA256 of file bytes using `sha2::Sha256`; compare to `seed_history` table; skip if unchanged
  - Upsert `seed_history` row after execution stub (`INSERT OR REPLACE`)
  - Execution stub is a no-op (actual DELETE/INSERT deferred to P7-G2b)
- Re-export `run` from `crates/anvilml-registry/src/lib.rs`
- Add `AnvilError::SeedMissingDirective(String)` variant in `anvilml-core/src/error.rs` with Display impl and test coverage update
- Integration tests verifying: table bootstrap idempotent, directive parsing hit/miss/error, SHA256 skip on unchanged file

### Out of Scope
- Actual seed execution logic (DELETE/INSERT) — stubbed as no-op, implemented in P7-G2b
- Seed strategy parsing beyond `replace_all` / `merge` recognition — parsed but not exercised until G2b
- CLI config integration for seeds path — handled in P7-G3
- Replacement of `SEED_ENTRIES` const with SeedLoader — handled in P7-G3

## Approach

1. **Add error variant.** Append `SeedMissingDirective(String)` to `AnvilError` enum in `crates/anvilml-core/src/error.rs`. Add the `Display` arm `"seed missing directive: {msg}"`. Update the `all_variants_display` test case list to include the new variant.

2. **Create `seed_loader.rs`.** Implement the module with these internal pieces:
   - `fn parse_header(bytes: &[u8]) -> Result<(String, String), AnvilError>` — reads first non-empty lines beginning with `-- anvil:`, extracts `seed_table` and `seed_strategy` (defaults to `replace_all` if absent). Returns error if `seed_table` is missing.
   - `fn compute_sha256(bytes: &[u8]) -> String` — uses `sha2::Sha256` and `hex::encode` to produce lowercase hex digest.
   - `pub async fn run(pool: &SqlitePool, seeds_dir: &Path) -> Result<(), AnvilError>` — the main entry point:
     a. Execute `CREATE TABLE IF NOT EXISTS seed_history(...)`.
     b. Read directory entries from `seeds_dir`, filter `.sql` files, sort by filename.
     c. For each file: read bytes → parse header → compute SHA256 → query `seed_history` for matching `filename`. If hash matches, continue to next file. Otherwise call the execution stub (no-op) then upsert `seed_history`.
   - Execution stub: a private async fn `execute_seed(pool, _table: &str, _body: &[u8], _strategy: &str)` that returns `Ok(())` without executing any SQL.

3. **Wire re-export.** Add `pub mod seed_loader;` and `pub use seed_loader::run;` to `crates/anvilml-registry/src/lib.rs`.

4. **Write integration tests.** Place in `crates/anvilml-registry/tests/seed_loader.rs` (integration test file). Use `open_in_memory()` for an isolated database per test. Test cases:
   - `test_table_bootstrap_idempotent`: call `run()` twice with same seeds dir; second call must not fail.
   - `test_directive_parsing_hit`: write a `.sql` file with both directives, call `run()`, verify seed_history row is created.
   - `test_directive_parsing_miss`: write a `.sql` file without `-- anvil:seed_table`, call `run()`, expect `Err(AnvilError::SeedMissingDirective)`.
   - `test_sha256_skip_unchanged`: first run with a seed file (creates row); modify nothing; second run must skip execution (stubbed so no side effect, but the hash comparison path is exercised).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-registry/src/seed_loader.rs` | New module: SeedLoader with run(), header parsing, SHA256, skip logic |
| Modify | `crates/anvilml-registry/src/lib.rs` | Add `pub mod seed_loader;` and `pub use seed_loader::run;` |
| Modify | `crates/anvilml-core/src/error.rs` | Add `SeedMissingDirective(String)` variant to AnvilError enum with Display impl and test update |
| Create | `crates/anvilml-registry/tests/seed_loader.rs` | Integration tests for seed loader functionality |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-registry/tests/seed_loader.rs` | `test_table_bootstrap_idempotent` | `CREATE TABLE IF NOT EXISTS seed_history` does not error on second run |
| `crates/anvilml-registry/tests/seed_loader.rs` | `test_directive_parsing_hit` | File with both directives is accepted and seed_history row created |
| `crates/anvilml-registry/tests/seed_loader.rs` | `test_directive_parsing_miss` | File missing `-- anvil:seed_table` returns `Err(SeedMissingDirective)` |
| `crates/anvilml-registry/tests/seed_loader.rs` | `test_sha256_skip_unchanged` | Second run with unchanged file skips execution path (hash match) |
| `crates/anvilml-core/src/error.rs` | `all_variants_display` (updated) | New variant produces valid non-empty Display string |

## CI Impact

No CI workflow changes required. The task only adds code within existing crates (`anvilml-registry`, `anvilml-core`). The existing CI gates (`cargo test --workspace --features mock-hardware`, `cargo clippy --workspace --features mock-hardware -- -D warnings`) will cover the new code. No new dependencies are introduced — `sha2` is already a workspace dependency used by `anvilml-registry`.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| `AnvilError::SeedMissingDirective` requires modifying `anvilml-core`, which is outside the listed "Files to create or modify" in the task description. | The error variant is explicitly referenced in the task's Key Implementation Notes (§3). Adding it is necessary for the code to compile and is a minimal one-line enum addition plus Display arm. Documented as a file affected. |
| `sha2` crate version compatibility — need `Sha256::digest` API. | `sha2` is already at version `"0.11"` in workspace dependencies. The `Sha256::digest` function is stable and takes `&[u8]`, returning a `GenericArray<u8, U32>`. Combined with `hex::encode` (already a dependency), this is straightforward. |
| Integration tests need temp `.sql` files on disk for directory enumeration. | Use `tempfile::TempDir` to create an isolated seeds directory per test, write `.sql` files into it, and let the loader enumerate from there. The database uses `open_in_memory()` so no filesystem persistence issues. |
| Seed file ordering — files must be processed in sorted filename order for deterministic behavior. | Use `ReadDir::read_dir()`, collect entries, sort by filename with `.sort_by_key(|e| e.file_name())` before processing. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-registry -- seed` exits 0 (all four integration tests pass)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `seed_history` table is created idempotently on every `run()` call
- [ ] Files missing `-- anvil:seed_table` directive cause a fatal `Err(AnvilError::SeedMissingDirective)`
- [ ] SHA256 comparison correctly skips execution when hash matches stored value
- [ ] `run` is re-exported from `anvilml-registry` crate root via `lib.rs`
