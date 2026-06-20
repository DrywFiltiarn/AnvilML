# Plan Report: P902-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P902-A1                                           |
| Phase       | 902 — ArtifactStore Relocation Retrofit           |
| Description | Create anvilml-artifacts crate; move store.rs verbatim; correct module doc |
| Depends on  | P15-A3                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-06-20T17:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create the `crates/anvilml-artifacts` crate as a new workspace member, move `artifact_store.rs` from `crates/anvilml-ipc/src/` to `crates/anvilml-artifacts/src/store.rs` verbatim (including all code and comments), create a `lib.rs` that re-exports `ArtifactStore`, and rewrite the module doc comment to replace the false "Why in `anvilml-ipc`?" cycle rationale with the correct shared-crate rationale documented in TASKS_PHASE902.md. The crate depends only on `anvilml-core` and the same I/O/database crates that `anvilml-ipc` already declares for this module. After completion, `cargo test -p anvilml-artifacts` exits 0.

## Scope

### In Scope
- Create `crates/anvilml-artifacts/` directory tree: `Cargo.toml`, `src/lib.rs`, `src/store.rs`, `tests/store_tests.rs`.
- Add `"crates/anvilml-artifacts"` to the workspace `members` list in root `Cargo.toml`.
- Move `crates/anvilml-ipc/src/artifact_store.rs` → `crates/anvilml-artifacts/src/store.rs` **verbatim** (byte-for-byte identical content, including all comments and doc strings — the ACT agent will then rewrite only the module-level doc comment per the text in this plan).
- Create `src/lib.rs` with `//!` crate doc, `pub mod store;`, and `pub use store::ArtifactStore;`.
- Rewrite the module-level doc comment in `store.rs`: remove the false "Why in `anvilml-ipc`?" section and replace it with the correct rationale from TASKS_PHASE902.md.
- Write integration tests in `tests/store_tests.rs` that exercise `save`, `get`, and `list` using an in-memory SQLite pool and a temp directory.
- Set crate version to `0.1.0`.

### Out of Scope
- Deleting `artifact_store.rs` from `anvilml-ipc` (P902-A2).
- Removing dead dependencies (`chrono`, `sha2`, `sqlx`) from `anvilml-ipc` (P902-A2).
- Updating import paths in `anvilml-scheduler` or `anvilml-server` (P902-A3, P902-A4).
- Updating `anvilml-server/src/lib.rs` crate doc (P902-A5).
- Any changes to `EventBroadcaster`, `ws/`, or other `anvilml-ipc` modules.

## Existing Codebase Assessment

**What exists:** `ArtifactStore` currently lives in `crates/anvilml-ipc/src/artifact_store.rs` (296 lines). It implements content-addressed PNG artifact storage with three public methods: `save()` (SHA-256 hash-based file write + SQLite INSERT OR IGNORE), `get()` (hash lookup returning `Option<PathBuf>`), and `list()` (optional job_id-filtered metadata retrieval). The struct holds a `PathBuf` for the artifact directory and a `sqlx::SqlitePool` for metadata storage. The module doc contains a false rationale claiming a dependency cycle between `anvilml-server` and `anvilml-scheduler` that does not exist.

**Established patterns:** The project follows a strict crate structure: `lib.rs` contains only `//!` crate doc, `pub mod`, and `pub use` (≤ 80 lines). Tests live in `crates/{name}/tests/` as separate test crates using the crate's public API. The `anvilml-registry` crate is the closest analogue: it has `lib.rs` → `store.rs`/`scanner.rs`/etc., uses `open_in_memory()` for test databases, `tempfile::tempdir()` for filesystem tests, and `serial_test` for env-var isolation. Dependencies use `{ workspace = true }` for shared crates and direct version pins for non-workspace deps like `sha2`.

**Gap:** The current `artifact_store.rs` has no tests (no `#[cfg(test)]` block, no separate test file). The new `anvilml-artifacts` crate must include tests to satisfy FORGE_AGENT_RULES §5.1 ("Every task that writes source code MUST include tests"). The test file will follow the `anvilml-registry/tests/store_tests.rs` pattern: in-memory SQLite, temp directory, `serial_test` for isolation.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | sha2    | 0.10            | Cargo.lock (MCP unavailable for rust-docs) | n/a |
| crate  | sqlx    | 0.9.0           | Workspace dep    | runtime-tokio, sqlite, json |
| crate  | tokio   | 1.52.3          | Workspace dep    | full |
| crate  | chrono  | 0.4.45          | Workspace dep    | serde |
| crate  | tracing | 0.1.44          | Workspace dep    | std, attributes |
| crate  | uuid    | 1.23.3          | Workspace dep    | serde, v4 |

All versions match the workspace `Cargo.toml` or the project's `Cargo.lock`. `sha2` is not a workspace dependency — it is declared directly as `"0.10"` in `anvilml-ipc` (and `anvilml-registry`), matching the task context exactly. Cargo.lock confirms `sha2 0.10.9` is the resolved version.

## Approach

1. **Create directory structure.** Create `crates/anvilml-artifacts/src/` and `crates/anvilml-artifacts/tests/` directories.

2. **Write `Cargo.toml`.** Create `crates/anvilml-artifacts/Cargo.toml` with:
   - `[package]`: `name = "anvilml-artifacts"`, `version = "0.1.0"`, `edition.workspace = true`.
   - `[dependencies]`: `anvilml-core = { path = "../anvilml-core" }`, `chrono = { workspace = true }`, `sha2 = "0.10"`, `sqlx = { workspace = true }`, `tokio = { workspace = true }`, `tracing = { workspace = true }`, `uuid = { workspace = true }`.
   - `[dev-dependencies]`: `serial_test = "3.5"`, `tempfile = { workspace = true }`, `tokio = { workspace = true, features = ["rt-multi-thread", "macros"] }`.

   Rationale: `serial_test` and `tempfile` are dev-deps only (used in tests), following the `anvilml-registry` pattern. `tokio` dev-dep adds `rt-multi-thread` and `macros` features needed for `#[tokio::test]` and `#[serial]` test attributes.

3. **Write `src/lib.rs`.** Create with:
   - `//!` crate-level doc: "Content-addressed PNG artifact storage for AnvilML. Stores generated images by SHA-256 hash and records metadata in SQLite. Shared by `anvilml-scheduler` and `anvilml-server`; neither owns the other's copy."
   - `pub mod store;`
   - `pub use store::ArtifactStore;`

   This follows the `anvilml-registry` lib.rs pattern exactly (crate doc + `pub mod` + `pub use`).

4. **Move `artifact_store.rs` → `store.rs`.** Copy `crates/anvilml-ipc/src/artifact_store.rs` to `crates/anvilml-artifacts/src/store.rs` verbatim. The file is 296 lines, contains no tests, and uses only types already verified (`ArtifactMeta` from `anvilml-core`, `Uuid` from `uuid`, `Utc` from `chrono`, `Sha256` from `sha2`, `sqlx::SqlitePool`, `tracing` macros).

5. **Rewrite module doc in `store.rs`.** Replace the module-level `//!` block (lines 1–29 of the original) with the corrected text from TASKS_PHASE902.md:

   > `ArtifactStore` lives in its own crate because it is shared by `anvilml-scheduler`
   > (which persists `WorkerEvent::ImageReady` payloads) and `anvilml-server` (which serves
   > artifacts over HTTP), and neither of those crates may depend on the other. This mirrors
   > `anvilml-registry`, which exists as its own crate for the same reason (shared by
   > `anvilml-worker` and `anvilml-scheduler`).

   Keep the remaining sections ("Idempotency", "Thread safety") unchanged as they are factual.

6. **Write `tests/store_tests.rs`.** Create integration tests that exercise the three public methods:
   - `test_save_and_get` — saves an artifact, verifies `get()` returns the correct path.
   - `test_save_idempotency` — saves the same bytes twice, verifies only one file on disk.
   - `test_list_all` — saves multiple artifacts, verifies `list(None)` returns all.
   - `test_list_filtered` — saves artifacts for two jobs, verifies `list(Some(job_id))` returns only matching.
   - `test_get_missing_hash` — verifies `get()` returns `None` for a nonexistent hash.

   Tests use `open_in_memory()` from `anvilml-registry` (or equivalently `sqlx::SqlitePool::connect_with(...)` with `memory:` URI) and `tempfile::tempdir()` for the artifact directory. Each test is annotated `#[serial]` for isolation.

7. **Update workspace members.** Append `"crates/anvilml-artifacts"` to the `members` array in root `Cargo.toml`.

8. **Verify compilation and tests.** Run `cargo check -p anvilml-artifacts --features mock-hardware` and `cargo test -p anvilml-artifacts` to confirm the crate builds and tests pass.

## Public API Surface

| Item | Crate/Module | Signature |
|------|-------------|-----------|
| `pub struct ArtifactStore` | `anvilml_artifacts::store` | `pub struct ArtifactStore { dir: PathBuf, db: SqlitePool }` |
| `pub async fn new` | `anvilml_artifacts::store::ArtifactStore` | `pub async fn new(dir: PathBuf, db: SqlitePool) -> Self` |
| `pub async fn save` | `anvilml_artifacts::store::ArtifactStore` | `pub async fn save(&self, job_id: Uuid, image_bytes: &[u8]) -> Result<ArtifactMeta>` |
| `pub async fn get` | `anvilml_artifacts::store::ArtifactStore` | `pub async fn get(&self, hash: &str) -> Result<Option<PathBuf>>` |
| `pub async fn list` | `anvilml_artifacts::store::ArtifactStore` | `pub async fn list(&self, job_id: Option<Uuid>) -> Result<Vec<ArtifactMeta>>` |
| `pub use store::ArtifactStore` | `anvilml_artifacts` (lib.rs) | Re-export at crate root |

No new types are introduced. All signatures match the existing `ArtifactStore` verbatim.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `Cargo.toml` | Add `"crates/anvilml-artifacts"` to workspace members |
| CREATE | `crates/anvilml-artifacts/Cargo.toml` | New crate manifest with deps from anvilml-ipc |
| CREATE | `crates/anvilml-artifacts/src/lib.rs` | Crate root: doc, `pub mod store`, `pub use` |
| CREATE | `crates/anvilml-artifacts/src/store.rs` | Moved verbatim from `anvilml-ipc/src/artifact_store.rs` with corrected module doc |
| CREATE | `crates/anvilml-artifacts/tests/store_tests.rs` | Integration tests for save/get/list/idempotency |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-artifacts/tests/store_tests.rs` | `test_save_and_get` | `save()` writes file and DB row; `get()` returns correct path | In-memory SQLite pool, temp dir | 128 bytes of PNG-like data, new job_id | `get()` returns `Some(path)` pointing to `{dir}/{hash}.png` | `cargo test -p anvilml-artifacts --test store_tests -- test_save_and_get --exact` exits 0 |
| `crates/anvilml-artifacts/tests/store_tests.rs` | `test_save_idempotency` | Saving identical bytes twice writes file only once | In-memory SQLite pool, temp dir | Same 128-byte data, two `save()` calls | Disk has exactly one file; DB has one row | `cargo test -p anvilml-artifacts --test store_tests -- test_save_idempotency --exact` exits 0 |
| `crates/anvilml-artifacts/tests/store_tests.rs` | `test_list_all` | `list(None)` returns all artifacts | In-memory SQLite pool, temp dir, 3 artifacts saved | 3 artifacts for 3 different jobs | `list(None)` returns Vec of 3 ArtifactMeta | `cargo test -p anvilml-artifacts --test store_tests -- test_list_all --exact` exits 0 |
| `crates/anvilml-artifacts/tests/store_tests.rs` | `test_list_filtered` | `list(Some(job_id))` returns only matching artifacts | In-memory SQLite pool, temp dir, 3 artifacts (2 for job A, 1 for job B) | 3 artifacts across 2 jobs | `list(Some(jobA))` returns 2; `list(Some(jobB))` returns 1 | `cargo test -p anvilml-artifacts --test store_tests -- test_list_filtered --exact` exits 0 |
| `crates/anvilml-artifacts/tests/store_tests.rs` | `test_get_missing_hash` | `get()` returns `None` for nonexistent hash | In-memory SQLite pool, temp dir | Hash string that was never saved | `get()` returns `None` | `cargo test -p anvilml-artifacts --test store_tests -- test_get_missing_hash --exact` exits 0 |

## CI Impact

No CI changes required. The new crate is added to the workspace members list, so `cargo test --workspace --features mock-hardware` (the GitHub CI `rust-linux` and `rust-windows` jobs) automatically includes it. No new file types, gates, or test modules are introduced — the test follows the established `crates/{name}/tests/` convention that CI already picks up.

## Platform Considerations

None identified. The `store.rs` code uses `std::fs::create_dir_all` (cross-platform, blocking) and `tokio::fs::write` (async, cross-platform). No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in ENVIRONMENT.md §7 (`cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu`) will exercise this crate automatically as a workspace member.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `sqlx` feature mismatch — the workspace declares `sqlx = { features = ["runtime-tokio", "sqlite", "json"] }` but `store.rs` uses `chrono::DateTime<Utc>` which requires sqlx's `chrono` feature for `row.get::<DateTime<Utc>, _>`. If `chrono` feature is not enabled on sqlx, the build fails. | Medium | High | Add `features = ["chrono"]` to the `sqlx` dep in anvilml-artifacts' Cargo.toml (same pattern as anvilml-registry's `sqlx = { workspace = true, features = ["chrono"] }`). The workspace base features are inherited; the additional `chrono` feature is additive. |
| Test uses `anvilml_registry::open_in_memory()` but that function is private (not pub). If `open_in_memory()` is not re-exported, tests cannot use it and must construct the pool directly via `sqlx::SqlitePool::connect_with(...)`. | Low | Medium | Check `anvilml_registry::lib.rs` for pub exports. If not available, write pool creation inline in tests using `sqlx::SqlitePool::options("memory:")`. This is a straightforward fallback. |
| Module doc rewrite inadvertently changes non-doc content (e.g., the `use` statements or struct definitions below the `//!` block). | Low | High | The module doc ends at line 29 (the `Thread safety` section). The `use` statements start at line 31. The edit is a targeted replacement of only lines 1–29 with the new text. Use exact string matching to avoid accidental changes. |

## Acceptance Criteria

- [ ] `cargo check -p anvilml-artifacts --features mock-hardware` exits 0
- [ ] `cargo test -p anvilml-artifacts` exits 0
- [ ] `head -1 crates/anvilml-artifacts/src/lib.rs` prints `//! Content-addressed PNG artifact storage`
- [ ] `grep "^pub mod store" crates/anvilml-artifacts/src/lib.rs` matches (lib.rs declares the module)
- [ ] `grep "^pub use store::ArtifactStore" crates/anvilml-artifacts/src/lib.rs` matches (lib.rs re-exports)
- [ ] `diff crates/anvilml-ipc/src/artifact_store.rs crates/anvilml-artifacts/src/store.rs` — lines 1–29 differ (module doc), lines 30–296 are identical
- [ ] `grep "crates/anvilml-artifacts" Cargo.toml` matches (workspace member)
- [ ] `grep "sha2" crates/anvilml-artifacts/Cargo.toml` matches (sha2 dependency present)
