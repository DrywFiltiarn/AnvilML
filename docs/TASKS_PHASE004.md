# Tasks: Phase 004 — Persistence & Model Registry

| Field            | Value                                                                     |
|------------------|---------------------------------------------------------------------------|
| Phase            | 004                                                                       |
| Name             | Persistence & Model Registry                                              |
| ANVIL Milestone  | M2 (part 1)                                                               |
| Status           | Draft                                                                     |
| Depends on phases| 1, 2, 3                                                                   |
| Task file        | `forge/tasks/tasks_phase004.json`                                         |
| Design reference | `ANVILML_DESIGN.md` §13 (SQLite Schema), §4.2 (Model/Artifact Types)     |

---

## Overview

Phase 004 implements `anvilml-registry`: the SQLite schema migrations, the filesystem scanner that discovers model files and derives their metadata, and the `ModelRegistry` store that persists and serves that metadata. No async runtime, HTTP server, or worker process is involved — this phase is pure data: SQL + file I/O + hashing.

This phase must precede the worker and scheduler phases because `ModelRegistry` is a shared dependency of both. The scheduler's DAG validation references `KNOWN_NODE_TYPES` (defined in phase 006), but model lookups at dispatch time go through the registry. The HTTP server in phases 007–008 serves `GET /v1/models` directly from the registry. Implementing either of those before the registry and its DB schema exist would require each to invent its own model store and then migrate, producing avoidable rework.

The three SQL migration files also establish the authoritative schema for the `jobs` and `artifacts` tables, which the scheduler and artifact store depend on. They are created here so that `sqlx::migrate!()` — which runs at server startup in phase 008 — has a stable, tested migration set from the beginning.

At the end of this phase: `cargo test -p anvilml-registry` exits 0 with at least 6 tests covering migration, scan, and store operations.

---

## Group Reference

| Group | Subsystem         | Tasks          | Summary                                              |
|-------|-------------------|----------------|------------------------------------------------------|
| A     | anvilml-registry  | P4-A1 … P4-A3  | Migrations, scanner, naming correction, ModelRegistry CRUD |

---

## Prerequisites

- P3-B1 complete: `anvilml-core` types including `ModelMeta`, `ModelKind`, `DType`, `ModelDirConfig`, `ArtifactMeta` are stable and exported.
- No SQLite tables exist yet. This phase creates them.

---

## Contract Documents Applicable to This Phase

| Document section          | Relevant tasks | What must match                                                     |
|---------------------------|----------------|---------------------------------------------------------------------|
| `ANVILML_DESIGN.md` §13   | P4-A1          | Exact SQL DDL for `jobs`, `models`, `artifacts` tables and indices  |
| `ANVILML_DESIGN.md` §4.2  | P4-A2, P4-A3   | `ModelMeta` field names, `ModelKind` variants, `DType` variants     |
| `ANVILML_DESIGN.md` §24.1 | P4-A2          | `ModelMeta.id` = first 16 hex chars of SHA256(canonical_path)      |

---

## Task Descriptions

### Group A — anvilml-registry

#### P4-A1: anvilml-registry — SQLite migrations (jobs, models, artifacts)

**Goal:** Create the three migration files that define the entire SQLite schema, and implement the pool initializer that applies them at startup.

**Files to create or modify:**
- `backend/migrations/001_jobs.sql` — `jobs` table + indices
- `backend/migrations/002_models.sql` — `models` table + index
- `backend/migrations/003_artifacts.sql` — `artifacts` table + index
- `crates/anvilml-registry/src/db.rs` — `pub async fn open(path: &Path) -> Result<SqlitePool, AnvilError>`
- `crates/anvilml-registry/src/lib.rs` — expose `db::open`
- `crates/anvilml-registry/Cargo.toml` — add `sqlx` (features: sqlite, runtime-tokio-native-tls, macros, migrate), `tokio` (features: full)

**Key implementation notes:**
- The exact DDL must match `ANVILML_DESIGN.md §13` character-for-character in column names and types. The scheduler (phase 006) and server (phases 007–008) run raw SQL and `sqlx::query!` macros against these tables; any deviation will produce compile-time or runtime errors in those later phases.
- `db::open` must set `PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA foreign_keys=ON;` immediately after the pool is created, before running migrations. Use `sqlx::migrate!("../backend/migrations")` — the path is relative to the crate root and must resolve correctly.
- The `sqlx` offline mode (`sqlx prepare`) is not required at this phase. The compile-time query check is opt-in. Add `[build-dependencies] sqlx = { features = ["offline"] }` only if compile-time checking is desired now; otherwise use runtime queries.
- Write a test: create a temporary file path, call `db::open(path)`, query `sqlite_master` and assert `jobs`, `models`, `artifacts` all appear.

**Acceptance criterion:** `cargo test -p anvilml-registry -- db` exits 0.

---

#### P4-A2: anvilml-registry — ModelMeta scanner

**Goal:** Implement the filesystem scanner that walks configured model directories, identifies model files by extension, derives all `ModelMeta` fields, and returns a `Vec<ModelMeta>` without touching the database.

**Files to create or modify:**
- `crates/anvilml-registry/src/scanner.rs` — `pub async fn scan_dirs(dirs: &[ModelDirConfig]) -> Vec<ModelMeta>`
- `crates/anvilml-registry/Cargo.toml` — add `walkdir`, `sha2`, `hex`, `chrono` (serde feature)

**Key implementation notes:**
- Walk each `ModelDirConfig.path` recursively using `walkdir`. Match files with extensions: `.safetensors`, `.ckpt`, `.pt`, `.bin`. Silently skip unreadable directories (log at `warn`, do not fail the scan).
- `ModelMeta.id`: canonicalize the path to an absolute path string, compute `SHA256`, take the first 16 hex characters. The canonical path string (not bytes) is the input so that path representations are stable across OS path separator differences.
- `ModelMeta.name`: the filename stem (no extension).
- `ModelMeta.kind`: if `ModelDirConfig.kind` is `Some(k)`, use it. Otherwise infer from the parent directory name using a simple case-insensitive match: `diffusion`, `unet` → `Diffusion`; `vae` → `Vae`; `lora` → `Lora`; `controlnet` → `ControlNet`; `clip` → `Clip`; `upscale` → `Upscale`; default `Diffusion`.
- `ModelMeta.dtype_hint`: scan the filename for substrings `fp16`, `f16` → `F16`; `bf16` → `BF16`; `q8` → `Q8`; `q4` → `Q4`; `fp32`, `f32` → `F32`; else `Unknown`.
- `ModelMeta.vram_estimate_mib`: `(size_bytes / 1024 / 1024) * factor` where factor by dtype: `F32=2.0`, `F16=1.0`, `BF16=1.0`, `Q8=0.5`, `Q4=0.25`, `Unknown=1.0`. Minimum 1 MiB.
- Write a test using a `tempdir` with two fixture files (create with `std::fs::write`) of known sizes and names. Assert the returned `ModelMeta` fields match expectations.

**Acceptance criterion:** `cargo test -p anvilml-registry -- scanner` exits 0.

---


---

#### P4-A2B: anvilml — naming correction (binary `anvilml`, database `anvilml.db`)

**Goal:** Apply the naming corrections introduced in `ANVILML_DESIGN.md` Rev 3 amendment before any code that hardcodes the binary or database name is written: the launcher binary is `anvilml` (not `sindristudio`) and the default database file is `anvilml.db` (not `sindristudio.db`).

**Why here.** This correction must land before P4-A3 (the first task that opens a real `SqlitePool` with a hardcoded default path) and before phase 008 (which produces the release binary). Inserting it here keeps all later tasks consistent from the start, avoids a rename-refactor mid-stream, and means the phrasing in phase 008 markdown (`./target/release/anvilml`) is already correct by the time it is written.

**Files to create or modify:**
- `backend/Cargo.toml` — set `[[bin]] name = "anvilml"` (replacing the previous `"sindristudio"` stub name if set)
- `crates/anvilml-core/src/config.rs` — `ServerConfig.db_path` default changed to `"./anvilml.db"`
- `anvilml.toml` — update `db_path = "./anvilml.db"`
- `backend/src/main.rs` — update the startup print to say `"AnvilML vX.Y.Z"` (not `"sindristudio"`)
- Code comments / doc comments that mention the binary name by its old value

**Key implementation notes:**
- The `[[bin]]` entry in `backend/Cargo.toml` determines the output filename in `target/release/`. After this task, `cargo build --release` must produce `target/release/anvilml` on Linux and `target/release/anvilml.exe` on Windows.
- The `db_path` default is set in `ServerConfig`'s `Default` implementation (or `#[serde(default)]` attribute). Change it from `"./sindristudio.db"` to `"./anvilml.db"`. All test helpers that create in-memory SQLite (`sqlite::memory:`) are unaffected — they do not use the default path.
- The `anvilml.toml` reference config (checked into the repo root) must reflect the new default so users copying it get `anvilml.db` out of the box.
- The binary name change does **not** affect any crate name, API path, IPC message field, environment variable, or config key — all of those remain `anvilml`-prefixed and are unaffected.
- `SindriStudio` (capitalised, the launcher product) is out of scope for this task. This task only corrects the lowercase binary and DB filename tokens.

**Acceptance criterion:**
```
cargo build --release
ls target/release/anvilml            # Linux
# or
dir target\release\anvilml.exe    # Windows
```
`cargo run -- --no-browser` creates `./anvilml.db`, not `./sindristudio.db`.

---

#### P4-A3: anvilml-registry — ModelRegistry store with upsert, list, get, rescan

**Goal:** Implement the `ModelRegistry` struct that wraps the SQLite pool and provides the model metadata CRUD operations used by the server and the scheduler.

**Files to create or modify:**
- `crates/anvilml-registry/src/store.rs` — `ModelRegistry` struct and all methods
- `crates/anvilml-registry/src/lib.rs` — `pub use store::ModelRegistry`

**Key implementation notes:**
- `ModelRegistry::new(pool: SqlitePool) -> Self`.
- `upsert(&self, meta: &ModelMeta) -> Result<(), AnvilError>`: `INSERT OR REPLACE INTO models ...` mapping all fields. `path` must be stored as the canonical absolute path string.
- `list(&self, kind: Option<ModelKind>) -> Result<Vec<ModelMeta>, AnvilError>`: `SELECT * FROM models` optionally filtered by `kind`. Return ordered by `name ASC`.
- `get(&self, id: &str) -> Result<Option<ModelMeta>, AnvilError>`: `SELECT * FROM models WHERE id = ?`.
- `rescan(&self, dirs: &[ModelDirConfig]) -> Result<(), AnvilError>`: call `scan_dirs(dirs)`, then bulk-upsert all results. The registry never auto-removes stale entries; removal is manual via future admin API. This keeps rescan non-destructive by default.
- Write tests: empty list returns empty vec; upsert then get returns correct meta; rescan with tempdir adds new entries; list with kind filter returns only matching entries.

**Acceptance criterion:** `cargo test -p anvilml-registry -- store` exits 0 with at least 4 tests.

---

## Phase Acceptance Criteria

```
cargo test -p anvilml-registry
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo fmt --all --check
```

---

## Known Constraints and Gotchas

- `sqlx::migrate!()` takes a path relative to the `CARGO_MANIFEST_DIR` of the crate calling it. `anvilml-registry`'s manifest is at `crates/anvilml-registry/Cargo.toml`; the migrations are at `backend/migrations/`. The correct argument is `sqlx::migrate!("../../backend/migrations")`. Verify this resolves correctly with a test that confirms all three tables exist after migration.
- `walkdir` follows symbolic links by default. If the user's model directory contains symlinks that form a cycle, the scan will loop. Set `follow_links(false)` unless the design explicitly requires symlink traversal.
- SHA256 of the canonical path string means the ID changes if the file is moved to a different absolute path. This is intentional — the ID is a location-stable handle for the current scan, not a content hash. The content hash is `ArtifactMeta.hash` (for output images), not `ModelMeta.id`.
- `sqlx` requires the `DATABASE_URL` environment variable to be set at compile time if using `query!` macros with compile-time checking. Either set it in a `.env` file at the workspace root during development, or use `query_unchecked!` / runtime queries for this phase to avoid the setup overhead.
