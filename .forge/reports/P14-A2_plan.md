# Plan Report: P14-A2

| Field       | Value                                                       |
|-------------|-------------------------------------------------------------|
| Task ID     | P14-A2                                                      |
| Phase       | 014 — Artifact Storage                                      |
| Description | anvilml-server: ArtifactStore.save (decode, hash, write, db insert) |
| Depends on  | P14-A1                                                      |
| Project     | anvilml                                                     |
| Planned at  | 2026-06-09T15:53:35Z                                        |
| Attempt     | 1                                                           |

## Objective

Add the `ArtifactStore` module to `anvilml-server` that implements content-addressed PNG artifact persistence: base64-decode an image, compute its SHA-256 hash, write it to a two-char-prefix-sharded directory under `artifact_dir`, insert the artifact metadata row into SQLite, and increment the job's `artifact_count`.

## Scope

### In Scope
- Add `sha2`, `hex`, `base64`, and `tokio::fs` (via `tokio/fs` feature) as dependencies in `anvilml-server/Cargo.toml`
- Create `crates/anvilml-server/src/artifact/mod.rs` (module entry point)
- Create `crates/anvilml-server/src/artifact/store.rs` with:
  - `ArtifactMeta` struct matching the design spec (§4.2, §13): `{ hash, job_id, width, height, format, seed, steps, prompt, created_at }`
  - `ArtifactStore` struct: `{ artifact_dir: PathBuf, db: SqlitePool }`
  - `ArtifactStore::new(artifact_dir, db)` constructor
  - `ArtifactStore::save(job_id, image_b64, meta_input) -> Result<ArtifactMeta>`:
    1. Base64-decode `image_b64` → raw PNG bytes
    2. `hash = hex::encode(sha2::Sha256::digest(bytes))`
    3. Create directory `{artifact_dir}/{hash[0..2]}` via `tokio::fs::create_dir_all`
    4. Write `{artifact_dir}/{hash[0..2]}/{hash}.png` via `tokio::fs::write`
    5. INSERT `ArtifactMeta` row into `artifacts` table using `sqlx::query`
    6. UPDATE `jobs.artifact_count = artifact_count + 1` for the given `job_id`
- Add unit test `artifact_save` in `crates/anvilml-server/tests/` that:
  - Creates a `tempfile::TempDir` for artifact storage
  - Opens an in-memory SQLite DB with migrations (via `anvilml_registry::open_in_memory`)
  - Inserts a placeholder job row into `jobs` table
  - Calls `ArtifactStore::save` with a known base64-encoded PNG string
  - Verifies the file exists on disk at the expected path
  - Verifies the `artifacts` table has one row with the correct hash
  - Verifies `jobs.artifact_count` was incremented from 0 to 1

### Out of Scope
- `GET /v1/artifacts/:hash` handler (P14-A4)
- `GET /v1/artifacts` list handler (P14-A5)
- `ArtifactStore::get_path`, `list`, `delete_for_job` (P14-A3 through P14-A5)
- Integration of `ArtifactStore` into `AppState` and scheduler (P14-A3)
- Updating the existing `ArtifactMeta` type in `anvilml-core` (left to a future cleanup task; this task defines its own local type matching the design spec)

## Approach

1. **Resolve dependency versions** using `rust-docs`:
   - `sha2` = 0.11 (already in workspace) — API: `sha2::{Sha256, Digest}`, `Sha256::digest(bytes)` returns `[u8; 32]`
   - `hex` = 0.4.3 (already in workspace) — API: `hex::encode(hash_bytes)` returns `String`
   - `base64` = 0.22.1 (new) — API: `base64::prelude::BASE64_STANDARD.decode(b64_str)` returns `Result<Vec<u8>, DecodeError>`
   - `tokio/fs` feature already available via workspace `tokio` (features: `full`)

2. **Update `anvilml-server/Cargo.toml`**:
   - Add `base64 = { workspace = true }` to `[dependencies]` (add `base64 = "0.22"` to workspace `[workspace.dependencies]` in root `Cargo.toml`)
   - Add `tokio = { workspace = true, features = ["macros", "rt-multi-thread", "sync", "time", "fs"] }` to include the `fs` feature for async file I/O

3. **Create `crates/anvilml-server/src/artifact/mod.rs`**:
   - `pub mod store;`
   - Re-export `ArtifactStore` from `store`

4. **Create `crates/anvilml-server/src/artifact/store.rs`**:
   - Define `ArtifactMeta` struct matching the design spec (§4.2, §13) with fields: `hash`, `job_id`, `width`, `height`, `format`, `seed`, `steps`, `prompt`, `created_at`
   - Define `ArtifactStoreInput` struct for the `meta_input` parameter carrying `width`, `height`, `seed`, `steps`, `prompt`
   - Implement `ArtifactStore::new(artifact_dir, db)`
   - Implement `ArtifactStore::save(job_id, image_b64, meta_input)`:
     - Decode base64 → bytes
     - Compute SHA-256 → hex hash string
     - `tokio::fs::create_dir_all` for prefix dir
     - `tokio::fs::write` for PNG file
     - `sqlx::query` INSERT into `artifacts` table
     - `sqlx::query` UPDATE `jobs` SET `artifact_count = artifact_count + 1` WHERE `id = ?`
     - Return constructed `ArtifactMeta`
   - Add `#[tracing::instrument]` on `save` with DEBUG logging of hash computation and file path

5. **Create test file `crates/anvilml-server/tests/api_artifact_save.rs`**:
   - Use `tempfile::TempDir` for artifact directory (auto-cleanup)
   - Use `anvilml_registry::open_in_memory()` for SQLite pool with migrations
   - Insert a test job row: `INSERT INTO jobs (id, status, graph, settings, artifact_count) VALUES (?, 'Queued', '{}', '{}', 0)`
   - Generate a minimal base64-encoded PNG (1x1 pixel) for the test input
   - Call `store.save(job_id, b64_png, meta_input)`
   - Assert: file exists at `{tmp_dir}/{hash[0..2]}/{hash}.png`
   - Assert: `SELECT COUNT(*) FROM artifacts` returns 1
   - Assert: `SELECT artifact_count FROM jobs WHERE id = ?` returns 1

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `Cargo.toml` (workspace root) | Add `base64 = "0.22"` to `[workspace.dependencies]` |
| Modify | `crates/anvilml-server/Cargo.toml` | Add `base64` dep; add `fs` feature to `tokio` |
| Create | `crates/anvilml-server/src/artifact/mod.rs` | Module entry point for artifact store |
| Create | `crates/anvilml-server/src/artifact/store.rs` | `ArtifactStore` struct and `save` method |
| Create | `crates/anvilml-server/tests/api_artifact_save.rs` | Unit test for `save` with tempdir + memory DB |
| Modify | `crates/anvilml-server/Cargo.toml` | Bump patch version `0.1.4 → 0.1.5` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-server/tests/api_artifact_save.rs` | `artifact_save` | Full save pipeline: decode → hash → write → DB insert → count increment |

## CI Impact

No CI workflow changes required. The existing `cargo test --workspace --features mock-hardware` command will pick up the new test file. The new `base64` dependency is a simple, well-tested crate with no platform-specific code.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ArtifactMeta` in `anvilml-core` has different fields than design spec | High | Medium | Define a local `ArtifactMeta` in `store.rs` matching the design spec; note the discrepancy for a future cleanup task |
| `base64` 0.22 API differs from older versions | Low | Low | Confirmed via docs.rs: `BASE64_STANDARD.decode()` returns `Result<Vec<u8>, DecodeError>` — straightforward to use |
| In-memory SQLite with migrations may not include `artifacts` table | Low | High | `anvilml_registry::open_in_memory()` runs all embedded migrations including `003_artifacts.sql` — verified in existing code |
| Job row missing from `jobs` table causes UPDATE to affect 0 rows | Medium | Low | Test inserts the job row before calling `save`; the UPDATE is not error-checked (0 rows = no job, which is expected) |
| `tempfile::TempDir` cleanup conflicts in parallel tests | Low | Low | Single test function; no parallelism needed |

## Acceptance Criteria

- [ ] `cargo check -p anvilml-server --features mock-hardware` exits 0
- [ ] `cargo test -p anvilml-server --features mock-hardware -- artifact_save` exits 0
- [ ] `cargo clippy -p anvilml-server --features mock-hardware -- -D warnings` exits 0
- [ ] `anvilml-server` version bumped from `0.1.4` to `0.1.5` in `Cargo.toml`
- [ ] New dependency `base64 = "0.22"` added to workspace and crate manifests
- [ ] `tokio` `fs` feature added to `anvilml-server` dependencies
- [ ] `ArtifactStore::save` decodes base64, computes SHA-256 hex hash, writes PNG to `{hash[0..2]}/{hash}.png`, inserts artifact row, increments `jobs.artifact_count`
