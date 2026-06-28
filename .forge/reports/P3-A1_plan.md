# Plan Report: P3-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P3-A1                                             |
| Phase       | 3 — Core Domain Types: Data Model                 |
| Description | anvilml-core: Job, JobStatus, JobSettings types   |
| Depends on  | P2-A7                                             |
| Project     | anvilml                                           |
| Planned at  | 2026-06-28T14:57:00Z                              |
| Attempt     | 1                                                 |

## Objective

Define the three job-related domain types (`Job`, `JobStatus`, `JobSettings`) specified
in `ANVILML_DESIGN.md §5.3` inside the new `types/` submodule of `anvilml-core`. This
gives the scheduler, IPC layer, and HTTP API a shared vocabulary for generation requests
and their lifecycle state. The deliverable is compile-ready Rust types with serde
serialisation, plus a dedicated integration test crate (`job_tests.rs`) with ≥4 tests
verifying serde roundtrips.

## Scope

### In Scope
- Create `crates/anvilml-core/src/types/mod.rs` declaring the `types` submodule tree.
- Create `crates/anvilml-core/src/types/job.rs` with `Job`, `JobStatus`, `JobSettings`.
- Modify `crates/anvilml-core/Cargo.toml` to add the `chrono` dependency.
- Modify `crates/anvilml-core/src/lib.rs` to declare `mod types;` and `pub use types::*;`.
- Create `crates/anvilml-core/tests/job_tests.rs` with ≥4 tests for serde roundtrips.

### Out of Scope
None. `defers_to (from JSON): []` — this task must implement its full scope with no
deferred functionality.

## Existing Codebase Assessment

`anvilml-core` currently has three source files: `config.rs`, `config_load.rs`, and
`error.rs`, plus a 11-line `lib.rs` that re-exports `ServerConfig`, `CliOverrides`,
`load`, and `AnvilError`. The `types/` submodule does not yet exist. The crate's
`Cargo.toml` already declares `uuid` (with `v4` and `serde` features) and
`serde_json` — both types used by the `Job` struct — but does **not** yet include
`chrono`, which is required for `DateTime<Utc>` fields.

The existing test files (`config_tests.rs`, `config_load_tests.rs`, `error_tests.rs`)
live in `crates/anvilml-core/tests/` as separate integration-test crates that import
the crate's public API. The project convention is one test file per source module
(`ANVILML_DESIGN.md §4.4`, `ENVIRONMENT.md §11.1`).

No dual-mode parity markers apply: `Job`, `JobStatus`, and `JobSettings` are pure data
types with no `execute()`, `load()`, `sample()`, or `decode()` method — the
`REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` convention (`ANVILML_DESIGN.md §10.6`) is
scoped to node and arch-module functions only.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | chrono  | 0.4.45          | rust-docs MCP  | serde                  |

`uuid` (1.23.4) and `serde_json` (1.0) are already present in `Cargo.toml`. The
`chrono` crate needs the `serde` feature so `DateTime<Utc>` fields derive
`Serialize`/`Deserialize` correctly.

## Approach

1. **Add `chrono` dependency to `Cargo.toml`.** Append a new line under `[dependencies]`:
   ```toml
   chrono = { version = "0.4", features = ["serde"] }
   ```
   Rationale: `uuid` and `serde_json` already exist; only `chrono` is missing. The
   `serde` feature is required because `Job` derives `Serialize`/`Deserialize` on
   `DateTime<Utc>` fields.

2. **Create `crates/anvilml-core/src/types/` directory and `mod.rs`.**
   Write `types/mod.rs` containing:
   ```rust
   pub mod job;
   ```
   This declares the `job` submodule. The file is intentionally minimal — subsequent
   Phase 3 tasks (P3-A2 through P3-A10) each add one `mod <name>;` line to this file.

3. **Create `crates/anvilml-core/src/types/job.rs`.** Define the three types per
   `ANVILML_DESIGN.md §5.3`:

   - `JobStatus` enum with variants `Queued`, `Running`, `Completed`, `Failed`,
     `Cancelled`. Derives: `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize`.
     Doc comment explaining the lifecycle.

   - `JobSettings` struct with field `device_preference: Option<String>`. Derives:
     `Debug, Clone, Serialize, Deserialize`. Doc comment on the struct and the field
     (the field doc is from the design doc: "Requested device. None = auto-select by VRAM.").

   - `Job` struct with fields `id: Uuid`, `status: JobStatus`,
     `graph: serde_json::Value`, `settings: JobSettings`, `created_at: DateTime<Utc>`,
     `started_at: Option<DateTime<Utc>>`, `completed_at: Option<DateTime<Utc>>`,
     `worker_id: Option<String>`, `error: Option<String>`,
     `queue_position: Option<u32>`. Derives: `Debug, Clone, Serialize, Deserialize`.
     Doc comment explaining what a Job represents.

   Imports at top of `job.rs`: `use chrono::Utc;` (for `DateTime<Utc>`),
   `use serde::{Serialize, Deserialize};`, `use uuid::Uuid;`.

   All three types include a `///` doc comment per `ANVILML_DESIGN.md §4.5` and
   `FORGE_AGENT_RULES.md §12.1`.

4. **Modify `crates/anvilml-core/src/lib.rs`.** Add two lines after the existing
   `mod error;`:
   ```rust
   pub mod types;
   pub use types::*;
   ```
   The `lib.rs` file will be 13 lines (well under the 80-line cap).

5. **Create `crates/anvilml-core/tests/job_tests.rs`.** Write ≥4 integration tests
   that exercise serde roundtrips:

   - `test_job_serde_roundtrip`: Serialize a `Job` with all fields populated to JSON,
     deserialize back, assert equality.
   - `test_job_status_all_variants_roundtrip`: For each of the five `JobStatus` variants,
     serialize to JSON and deserialize back, assert equality.
   - `test_job_settings_default`: Serialize a default-constructed `JobSettings` and
     verify the JSON contains `"device_preference": null`.
   - `test_job_with_nulls_roundtrip`: Serialize a `Job` with `started_at`,
     `completed_at`, `worker_id`, `error`, and `queue_position` all `None`, deserialize,
     assert the `None` fields round-trip correctly.

   Each test is a standalone function in the test crate, importing types via
   `use anvilml_core::types::*;` (or fully qualified paths).

6. **Verify compilation and tests.** Run `cargo test -p anvilml-core --test job_tests`
   and confirm exit 0 with ≥4 tests passing.

## Public API Surface

| Item | Crate/Module Path | Signature |
|------|-------------------|-----------|
| `Job` | `anvilml_core::types::Job` | `pub struct Job { id: Uuid, status: JobStatus, graph: serde_json::Value, settings: JobSettings, created_at: DateTime<Utc>, started_at: Option<DateTime<Utc>>, completed_at: Option<DateTime<Utc>>, worker_id: Option<String>, error: Option<String>, queue_position: Option<u32> }` |
| `JobStatus` | `anvilml_core::types::JobStatus` | `pub enum JobStatus { Queued, Running, Completed, Failed, Cancelled }` |
| `JobSettings` | `anvilml_core::types::JobSettings` | `pub struct JobSettings { pub device_preference: Option<String> }` |
| `mod types` | `anvilml_core::types` | `pub mod types;` (module declaration) |

All three types are `pub` and re-exported via `pub use types::*;` in `lib.rs`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/types/mod.rs` | New module declaration file for the `types` submodule tree. |
| CREATE | `crates/anvilml-core/src/types/job.rs` | `Job`, `JobStatus`, `JobSettings` type definitions. |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Add `chrono` dependency with `serde` feature. |
| MODIFY | `crates/anvilml-core/src/lib.rs` | Add `pub mod types;` and `pub use types::*;`. |
| CREATE | `crates/anvilml-core/tests/job_tests.rs` | Integration tests for serde roundtrips (≥4 tests). |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-core/tests/job_tests.rs` | `test_job_serde_roundtrip` | Full `Job` with all fields populated serialises and deserialises back to an equal value. | Types exist and compile. | `Job` with `Uuid`, `JobStatus::Queued`, graph JSON, `JobSettings { device_preference: Some("cuda") }`, timestamps, `worker_id`, `error`, `queue_position`. | Roundtripped `Job` equals original. | `cargo test -p anvilml-core --test job_tests -- test_job_serde_roundtrip` exits 0 |
| `crates/anvilml-core/tests/job_tests.rs` | `test_job_status_all_variants_roundtrip` | Each of the five `JobStatus` variants roundtrips correctly through serde JSON. | `JobStatus` exists with all five variants. | `JobStatus::Queued`, `Running`, `Completed`, `Failed`, `Cancelled`. | Each variant serialises to expected JSON string and deserialises back to equal. | `cargo test -p anvilml-core --test job_tests -- test_job_status_all_variants_roundtrip` exits 0 |
| `crates/anvilml-core/tests/job_tests.rs` | `test_job_settings_default` | A `JobSettings` with `device_preference: None` serialises to `"device_preference": null`. | `JobSettings` exists. | `JobSettings { device_preference: None }`. | JSON contains `"device_preference": null`; deserialises back to equal. | `cargo test -p anvilml-core --test job_tests -- test_job_settings_default` exits 0 |
| `crates/anvilml-core/tests/job_tests.rs` | `test_job_with_nulls_roundtrip` | A `Job` with all `Option` fields set to `None` roundtrips correctly. | `Job` exists. | `Job` with `started_at: None`, `completed_at: None`, `worker_id: None`, `error: None`, `queue_position: None`. | All `None` fields remain `None` after roundtrip; non-null fields unchanged. | `cargo test -p anvilml-core --test job_tests -- test_job_with_nulls_roundtrip` exits 0 |

## CI Impact

No CI changes required. The task adds a new integration test file under
`crates/anvilml-core/tests/`, which is automatically picked up by the existing
`cargo test --workspace --features mock-hardware` CI job. No new CI gates, file types,
or build configuration are introduced.

## Platform Considerations

None identified. The `Job`, `JobStatus`, and `JobSettings` types are pure data with no
platform-specific logic, no `#[cfg(unix)]`/`#[cfg(windows)]` guards, and no filesystem
or network operations. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `chrono` version 0.4.45 may have a different `DateTime<Utc>` serde API than assumed (e.g. `#[serde(with = "chrono::serde::ts_seconds")]` attribute required). | Low | Medium | The `chrono` crate's `serde` feature re-exports the standard `DateTime` serialization via the `chrono::serde` module automatically when the `serde` feature is enabled — no extra attributes needed. Verified via MCP lookup of chrono 0.4.45 feature flags. |
| Adding `chrono` as a dependency may trigger a transitive dependency conflict with existing crates (e.g. `uuid` already pulls in `chrono` in some feature combinations). | Low | Low | `uuid` does not depend on `chrono` by default; `serde_json` also has no `chrono` dependency. The workspace will resolve cleanly. If a conflict arises, the ACT agent will run `cargo update` to let cargo resolve it. |
| `lib.rs` exceeds 80 lines after adding `pub mod types;` and `pub use types::*;`. | Very Low | Medium | `lib.rs` is currently 11 lines. Adding 2 lines results in 13 lines, well under 80. This risk is effectively theoretical. |

## Acceptance Criteria

- [ ] `cargo build -p anvilml-core` exits 0
- [ ] `cargo test -p anvilml-core --test job_tests` exits 0 with ≥4 tests
- [ ] `grep "^pub mod\|^pub use" crates/anvilml-core/src/lib.rs | wc -l` returns ≥ 2 (confirms `mod types` and `pub use types::*;` are present)
- [ ] `wc -l crates/anvilml-core/src/lib.rs` returns ≤ 80
