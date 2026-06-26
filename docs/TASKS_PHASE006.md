# Tasks: Phase 6 — Model Registry & Artifacts

**Phase:** 6
**Name:** Model Registry & Artifacts
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 2, 3

---

## Overview

This phase builds `anvilml-registry` (SQLite persistence, model directory scanning,
the PCI-ID device capability hint table, and the one-time seed loader) and
`anvilml-artifacts` (content-addressed PNG storage). Both crates depend only on
`anvilml-core`'s domain types (Phase 3) and have no dependency on hardware detection,
IPC, or the worker pool — this is why they can be built now, in parallel with the IPC
work that follows in Phases 7–8, rather than waiting on it.

This phase exists at this point in the sequence because the model registry and
artifact store are exactly the kind of "core infrastructure" the `ANVILML_DESIGN.md
§20` roadmap groups together with config, hardware detection, and domain types — all
of it is state and persistence machinery the scheduler and server will depend on
later, none of it requires a running worker subprocess to test. `anvilml-registry`
also introduces the first real `SqlitePool` and migration files in the repository,
which `anvilml-artifacts` then reuses rather than duplicating its own database setup.

At the start of this phase, neither crate has any implementation beyond Phase 1's
empty stub. At the end: `anvilml-registry` can scan a model directory into SQLite,
persist and query `ModelMeta` rows, look up PCI-ID capability hints, and idempotently
apply one-time SQL seed files; `anvilml-artifacts` can save and retrieve
content-addressed PNG artifacts. Neither crate's capability is yet wired into an HTTP
endpoint — `/v1/models` and `/v1/artifacts` are the HTTP server phase's scope, not
this one — so this phase's tests are the complete proof of its deliverable, with no
Runnable Proof against a live server.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Model Registry | P6-A1 … P6-A8 | Migrations, `SqlitePool` creation, `ModelStore`, `ModelScanner`, `DeviceCapabilityStore`, `SeedLoader`, `lib.rs` cleanup |
| B | Artifacts | P6-B1 … P6-B3 | `ArtifactStore`'s `save()`, the artifacts table migration, `get()`, and `list()` |

---

## Prerequisites

`anvilml-core` must export `ModelMeta`, `ModelKind`, `ModelDtype`, `ModelFormat`,
`ArtifactMeta`, `InferenceCaps`, and `AnvilError` exactly as defined in Phase 3
(P3-A2, P3-A3, P3-A5, P3-A11). `anvilml-registry` and `anvilml-artifacts` must exist
as buildable stub crates per Phase 1's P1-B3.

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §7.1` | P6-A1 … P6-A8 | `anvilml-registry`'s module layout (`db.rs`, `scanner.rs`, `store.rs`, `device_store.rs`, `seed_loader.rs`) |
| `ANVILML_DESIGN.md §7.2`–§7.4 | P6-A4 | Model ID derivation (SHA256 of first 1 MiB), `ModelKind`/`ModelDtype` inference rules |
| `ANVILML_DESIGN.md §7.5` / `SUPPORTED_DEVICES_DB.md` migration DDL reference | P6-A1, P6-A5, P6-A6, P6-A7 | `device_capabilities` table schema; the one-time, frozen nature of the Markdown→SQL conversion (not this phase's scope to perform) |
| `ANVILML_DESIGN.md §3.3` | P6-B1 | `anvilml-artifacts` shares `anvilml-registry`'s `SqlitePool`, never owns its own `db.rs` |

---

## Task Descriptions

### Group A — Model Registry

#### P6-A1: database/: migrations dir + 001_initial.sql (models, device_capabilities)

**Goal:** Create the first migration file, establishing both tables the registry
needs before any Rust code can run a query against them.

**Files to create or modify:**
- `database/migrations/001_initial.sql` — `models` and `device_capabilities` tables
  plus the unique index on the latter's composite PCI-ID key.

**Key implementation notes:**
- `device_capabilities`'s schema matches the migration DDL reference in
  `SUPPORTED_DEVICES_DB.md` exactly — boolean columns are `INTEGER 0/1`, not a SQLite
  `BOOLEAN` type (SQLite has none natively).
- No `jobs` or `artifacts` tables here — `jobs` belongs to the scheduler phase;
  `artifacts` is this phase's own P6-B2, deliberately a separate migration file.

**Acceptance criterion:**
```bash
sqlite3 :memory: < database/migrations/001_initial.sql
# -> exit 0
```

#### P6-A2: anvilml-registry: db.rs SqlitePool creation + migration runner

**Goal:** Implement the pool-creation and migration-running entry point every other
task in this phase's Group A depends on to get a working `SqlitePool`.

**Files to create or modify:**
- `crates/anvilml-registry/src/db.rs` — `create_pool()`.
- `crates/anvilml-registry/Cargo.toml` — adds `sqlx` with `sqlite`/`runtime-tokio`
  features.

**Key implementation notes:**
- WAL mode is enabled on every created pool (`PRAGMA journal_mode=WAL`).
- Ghost-job reset (`ANVILML_DESIGN.md §19.2`) is explicitly **not** implemented
  here — it requires a `jobs` table that doesn't exist until the scheduler phase.
  Do not add a no-op stub for it; it simply doesn't apply yet.

**Acceptance criterion:**
```bash
cargo test -p anvilml-registry --test db_tests
# -> >=4 tests, exits 0
```

#### P6-A3: anvilml-registry: ModelStore CRUD for ModelMeta

**Goal:** Implement the persistence layer for `ModelMeta`, the type every later
consumer (the scanner, the future `/v1/models` handler) reads and writes through.

**Files to create or modify:**
- `crates/anvilml-registry/src/store.rs` — `ModelStore`.

**Key implementation notes:**
- `mtime_unix` and `scanned_at` are extra columns on the `models` table, populated
  by the scanner (next task) — they are not fields added to `anvilml-core`'s
  `ModelMeta` struct itself, since that struct's shape is fixed by Phase 3 and this
  phase doesn't reopen it.
- Every test uses its own in-memory database connection per `ANVILML_DESIGN.md
  §17.4` rule 1 — no shared fixture database across tests in this file.

**Acceptance criterion:**
```bash
cargo test -p anvilml-registry --test store_tests
# -> >=5 tests, exits 0
```

#### P6-A4: anvilml-registry: ModelScanner hashing + ModelKind/Dtype inference

**Goal:** Implement the directory-walking scanner that derives `ModelMeta` from
real files on disk, the piece that actually populates the registry from a model
directory.

**Files to create or modify:**
- `crates/anvilml-registry/src/scanner.rs` — `ModelScanner`.

**Key implementation notes:**
- Model ID is the lowercase hex SHA256 of the file's first 1 MiB (or the whole file
  if smaller) — chosen specifically for speed on large files and stability across
  renames, per `ANVILML_DESIGN.md §7.2`.
- `ModelKind` inference reads the directory component relative to the model root
  (`diffusion/`→`Diffusion`, `text_encoders/`→`TextEncoder`, `vae/`→`Vae`, anything
  else→`Unknown`) — `Lora`/`ControlNet`/`Upscale` have no scanner mapping in the MVP
  even though the enum variants exist, per `ANVILML_DESIGN.md §7.3`.
- A file already in the store with unchanged size and mtime is not re-hashed —
  the scanner takes a `ModelStore` reference to check this before hashing.

**Acceptance criterion:**
```bash
cargo test -p anvilml-registry --test scanner_tests
# -> >=6 tests, exits 0
```

#### P6-A5: anvilml-registry: DeviceCapabilityStore PCI-ID lookup

**Goal:** Implement the read-only query layer over the `device_capabilities` table
that `anvilml-hardware`'s future detection orchestration will eventually call (once
`detect_all_devices()` gains its `SqlitePool` parameter in a later phase).

**Files to create or modify:**
- `crates/anvilml-registry/src/device_store.rs` — `DeviceCapabilityStore`.

**Key implementation notes:**
- `lookup()` returns `None`, never `Err`, for an unknown PCI-ID pair — the caller is
  responsible for falling through to `CapabilitySource::Fallback`; this store never
  invents a fallback row of its own, per `ANVILML_DESIGN.md §7.5`.
- No write methods exist in MVP scope — this table is populated exactly once, by the
  seed loader (next two tasks), never written to by application code at runtime.

**Acceptance criterion:**
```bash
cargo test -p anvilml-registry --test device_store_tests
# -> >=4 tests, exits 0
```

#### P6-A6: anvilml-registry: SeedLoader hash-check + bookkeeping table

**Goal:** Implement the idempotency-check half of the seed loader — the bookkeeping
table and hash-comparison logic that lets `run()` (next task) skip work safely when
nothing has changed.

**Files to create or modify:**
- `crates/anvilml-registry/src/seed_loader.rs` — `SeedLoader::new()`,
  `already_applied()`, and the `_seed_log` bookkeeping table.

**Key implementation notes:**
- `_seed_log` is created by this module, not by `001_initial.sql` — it's this
  module's own internal bookkeeping concern, not part of the application schema.
- `already_applied()` returns `true` only when both the `seed_name` exists **and**
  its recorded hash matches the given one — a changed file is correctly treated as
  not yet applied, triggering a re-run.
- Actual SQL execution is explicitly out of scope here; this task only implements
  the check, not the act of applying anything.

**Acceptance criterion:**
```bash
cargo test -p anvilml-registry --test seed_loader_tests
# -> >=3 tests, exits 0
```

#### P6-A7: anvilml-registry: SeedLoader::run() SQL execution + recording

**Goal:** Complete the seed loader with the actual SQL-execution path, making it a
usable, idempotent, one-time runner for `database/seeds/devices.sql` at server
startup.

**Files to create or modify:**
- `crates/anvilml-registry/src/seed_loader.rs` — adds `run()`.

**Key implementation notes:**
- This receives exactly the execution scope P6-A6 deferred — confirm
  `already_applied()` exists and is correct before building on it.
- The seed file itself (`database/seeds/devices.sql`, populated with real PCI-ID
  rows converted from `docs/SUPPORTED_DEVICES_DB.md`) is a separate, one-time
  data-conversion task explicitly **outside** this phase's scope, per
  `ANVILML_DESIGN.md §7.5` — this task only builds the runner, not the data.

**Acceptance criterion:**
```bash
cargo test -p anvilml-registry --test seed_loader_tests
# -> >=7 tests total in the file, exits 0
```

#### P6-A8: anvilml-registry: lib.rs re-export pass, 80-line check

**Goal:** Finalize `anvilml-registry`'s public surface, confirming every module from
this phase is correctly re-exported and the crate's `lib.rs` stays within the
80-line hard cap.

**Files to create or modify:**
- `crates/anvilml-registry/src/lib.rs` — re-exports only.

**Key implementation notes:**
- Same pattern as every prior crate's closing `lib.rs` task (P1-B1, P3-A11, P5-A4) —
  no implementation logic, re-export and line-count verification only.

**Acceptance criterion:**
```bash
wc -l crates/anvilml-registry/src/lib.rs
# -> <=80
cargo test -p anvilml-registry
# -> exits 0, full crate suite
```

---

### Group B — Artifacts

#### P6-B1: anvilml-artifacts: ArtifactStore::save content-addressed write

**Goal:** Implement the write path for generated PNG artifacts, establishing
content-addressed storage before the read path (next task) or listing (the task
after that) are built on top of it.

**Files to create or modify:**
- `crates/anvilml-artifacts/src/store.rs` — `ArtifactStore::save()`.

**Key implementation notes:**
- The content hash (SHA256 hex of the PNG bytes) is computed here, not passed in —
  this is the artifact's primary key and filename.
- A duplicate `save()` (identical hash) is a no-op write, not an error — the file is
  simply already present.
- `anvilml-artifacts` has no `db.rs` of its own per `ANVILML_DESIGN.md §3.1` — it
  shares the `SqlitePool` that `anvilml-registry`'s `db.rs` creates, passed in by
  whatever code constructs the `ArtifactStore`.
- No migration is added in this task — the `artifacts` table doesn't exist until
  P6-B2, which this task's `defers_to` explicitly hands that scope to.

**Acceptance criterion:**
```bash
cargo test -p anvilml-artifacts --test store_tests
# -> >=3 tests, exits 0
```

#### P6-B2: database/: artifacts table migration + ArtifactStore::get

**Goal:** Add the `artifacts` table and implement the read path, completing the
save/get round trip for a single artifact by hash.

**Files to create or modify:**
- `database/migrations/002_artifacts.sql` — `artifacts` table + `job_id` index.
- `crates/anvilml-artifacts/src/store.rs` — adds `get()`.
- `crates/anvilml-artifacts/src/lib.rs` — `pub use store::ArtifactStore;`.

**Key implementation notes:**
- This receives exactly the migration dependency P6-B1 deferred.
- `get()` is a pure filesystem read by hash — it does not need to query the new
  `artifacts` table at all for this method; the table exists for metadata and for
  the next task's `list()`.

**Acceptance criterion:**
```bash
sqlite3 :memory: < database/migrations/002_artifacts.sql
cargo test -p anvilml-artifacts --test store_tests
# -> >=6 tests total in the file, exits 0
```

#### P6-B3: anvilml-artifacts: ArtifactStore::list by job_id

**Goal:** Complete `ArtifactStore` with a listing method, the piece the future
`GET /v1/artifacts?job_id=` handler will call.

**Files to create or modify:**
- `crates/anvilml-artifacts/src/store.rs` — adds `list()`.

**Key implementation notes:**
- This receives exactly the listing scope P6-B2 deferred.
- `list(None)` returns every row; `list(Some(job_id))` filters — both paths are
  required test cases.

**Acceptance criterion:**
```bash
cargo test -p anvilml-artifacts --test store_tests
# -> >=9 tests total in the file, exits 0
```

---

## Phase Acceptance Criteria

```bash
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test --workspace --features mock-hardware

# Runnable Proof: not applicable — this phase implements persistence-layer crates
# (anvilml-registry, anvilml-artifacts) with no HTTP handler, CLI subcommand, or
# other externally observable surface wired up yet. /v1/models and /v1/artifacts
# are the HTTP server phase's scope, not this one. The full test suite (db_tests,
# store_tests, scanner_tests, device_store_tests, seed_loader_tests for
# anvilml-registry; store_tests for anvilml-artifacts) is the complete and
# sufficient proof of this phase's deliverable, per the narrow exemption in
# FORGE_TASK_AUTHORING_SPEC.md §9.
```

---

## Known Constraints and Gotchas

- `anvilml-artifacts` must never gain its own `db.rs` or independent `SqlitePool`
  construction — it always shares the pool `anvilml-registry`'s `db.rs` creates, per
  `ANVILML_DESIGN.md §3.3`'s explicit "owned independently... neither owns the
  other's copy" framing, which refers to crate ownership of the *artifact data*, not
  to each crate maintaining a separate database connection.
- `docs/SUPPORTED_DEVICES_DB.md` is never read, parsed, or regenerated by any task in
  this phase (or any phase) — it is a permanent human-reference document, frozen
  after a one-time hand conversion that is explicitly out of this phase's scope. Do
  not author that conversion task as part of this phase's work.
- `_seed_log`'s table creation belongs in `seed_loader.rs`, not in
  `001_initial.sql` — it is the seed loader's own bookkeeping concern, distinct from
  the application's domain schema.
- Ghost-job reset is correctly absent from this phase — it requires a `jobs` table
  that doesn't exist until the scheduler phase. Its absence here is not an oversight.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 6 — Model Registry & Artifacts

**Capability proved:** Not applicable — this phase implements `anvilml-registry` and
`anvilml-artifacts` as persistence-layer crates with no HTTP handler or other
externally observable surface wired up yet. See `TASKS_PHASE006.md`'s Phase
Acceptance Criteria for the full test-suite proof.
```
