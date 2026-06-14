# Plan Report: P3-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P3-A1                                             |
| Phase       | 003 — Core Domain Types                           |
| Description | anvilml-core: job types (Job, JobStatus, JobSettings, SubmitJobRequest/Response) |
| Depends on  | P2-B1 (ServerConfig exists in anvilml-core)       |
| Project     | anvilml                                           |
| Planned at  | 2026-06-14T15:50:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create `crates/anvilml-core/src/types/job.rs` with the `Job`, `JobStatus`, `JobSettings`, `SubmitJobRequest`, and `SubmitJobResponse` types per `ANVILML_DESIGN.md §5.3`, wire the new `types` module into `lib.rs`, and add the `chrono` and `utoipa` workspace dependencies required by these types. When complete, `cargo test -p anvilml-core -- types::job` exits 0 with ≥ 3 tests, confirming JSON roundtrip correctness, default impl validity, and all five `JobStatus` variants serialise/deserialise correctly.

## Scope

### In Scope
- **CREATE** `crates/anvilml-core/src/types/mod.rs` — module declaration and re-exports for the types submodule.
- **CREATE** `crates/anvilml-core/src/types/job.rs` — all five job types:
  - `Job` struct with fields: `id: Uuid`, `status: JobStatus`, `graph: serde_json::Value`, `settings: JobSettings`, `created_at: DateTime<Utc>`, `started_at: Option<DateTime<Utc>>`, `completed_at: Option<DateTime<Utc>>`, `worker_id: Option<String>`, `error: Option<String>`, `queue_position: Option<u32>`.
  - `JobStatus` enum: `Queued`, `Running`, `Completed`, `Failed`, `Cancelled`.
  - `JobSettings` struct with field: `device_preference: Option<String>`.
  - `SubmitJobRequest` struct with fields: `graph: serde_json::Value`, `settings: JobSettings`.
  - `SubmitJobResponse` struct with fields: `job_id: Uuid`, `queue_position: u32`.
  - All types derive `Serialize`, `Deserialize`, `Clone`, `Debug`, and `ToSchema`.
  - `JobStatus` additionally derives `Copy`, `PartialEq`, `Eq`.
  - `JobSettings` derives `Default`.
  - `SubmitJobRequest` derives `Default`.
  - `SubmitJobResponse` derives `Default`.
  - Field-level `///` doc comments on all public items.
- **MODIFY** `crates/anvilml-core/src/lib.rs` — add `pub mod types;` and `pub use types::{Job, JobStatus, JobSettings, SubmitJobRequest, SubmitJobResponse};`.
- **MODIFY** `crates/anvilml-core/Cargo.toml` — add `chrono` (with `serde` feature) and `utoipa` (with `macros` and `chrono` features) as workspace dependencies.
- **MODIFY** `Cargo.toml` (workspace root) — add `chrono` and `utoipa` to `[workspace.dependencies]`.
- **CREATE** `crates/anvilml-core/tests/job_tests.rs` — ≥ 3 tests: JSON roundtrip, default impl, status variants.

### Out of Scope
- `types/model.rs`, `types/artifact.rs`, `types/hardware.rs`, `types/node.rs`, `types/worker.rs`, `types/events.rs` — these are separate tasks (P3-A2 through P3-A5).
- Any scheduler, handler, or persistence code that consumes these types.
- Database schema changes for job storage.
- WebSocket event variants for job state transitions (covered in P3-A5).

## Existing Codebase Assessment

The `anvilml-core` crate currently has four source files: `lib.rs` (20 lines, clean re-exports only), `config.rs` (216 lines, `ServerConfig` and nested config structs with `Default` impls and `path_as_string` helpers), `config_load.rs` (136 lines, `load()` function with four-level precedence), and `error.rs` (38 lines, minimal `AnvilError` with three variants). The `types/` directory does not yet exist — this task creates it from scratch.

The established patterns are clear:
- **Derive chain**: `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]` (or `Copy` where appropriate). `Default` is implemented manually for structs with non-trivial defaults, or via `#[derive(Default)]` when all fields have sensible defaults.
- **Doc comments**: Every `pub` struct/enum/field has a `///` doc comment describing purpose and non-obvious invariants.
- **Module structure**: `lib.rs` declares `pub mod` and re-exports via `pub use`. The `config` and `config_load` modules follow this pattern.
- **Test style**: Integration tests live in `crates/anvilml-core/tests/` as separate test crate files. Tests use `use anvilml_core::...` imports (not `crate::`). Each test has a `///` doc comment. Tests verify defaults, roundtrip serialisation, and non-default value preservation.
- **Environment variable isolation**: Tests that set `std::env::set_var` capture and restore the prior value unconditionally.

The design doc specifies `DateTime<Utc>` for timestamps but no `chrono` dependency exists in the workspace. The design doc also requires `ToSchema` from `utoipa` but no `utoipa` dependency exists. Both are new dependencies introduced by this task.

## Resolved Dependencies

| Type   | Name     | Version verified | MCP source     | Feature flags confirmed |
|--------|----------|-----------------|----------------|------------------------|
| crate  | chrono   | 0.4.45          | crates.io API  | serde (for DateTime<Utc> Serialize/Deserialize) |
| crate  | utoipa   | 5.5.0           | crates.io API  | macros (default, provides ToSchema derive), chrono (enables chrono type support in utoipa-gen) |

Note: No `rust-docs` MCP tool was available in this session. Versions were resolved via `webfetch` against `https://crates.io/api/v1/crates/{name}`. The `serde` feature for `chrono` is required because `DateTime<Utc>` does not implement `Serialize`/`Deserialize` without it (it is NOT in chrono's default features). The `macros` feature for `utoipa` is the default and provides the `ToSchema` derive macro. The `chrono` feature on `utoipa` enables chrono type support in `utoipa-gen`.

The task context mentions `utoipa` with the `axum` feature, but utoipa 5.5.0 does not have an `axum` feature — it has `axum_extras` (which enables axum type compatibility like `StatusCode` in schemas). Since this task only needs `ToSchema` on domain types (not axum-specific types), the `macros` and `chrono` features are sufficient. If the `axum` feature was intended for a different purpose, the ACT agent should confirm at session start.

## Approach

1. **Add workspace dependencies.** In `Cargo.toml` (workspace root), add two entries to `[workspace.dependencies]`:
   - `chrono = { version = "0.4.45", features = ["serde"] }` — the `serde` feature is required for `DateTime<Utc>` to implement `Serialize` and `Deserialize`.
   - `utoipa = { version = "5.5.0", features = ["macros", "chrono"] }` — `macros` is the default feature that provides `ToSchema`; `chrono` enables chrono type support in `utoipa-gen` so `DateTime<Utc>` gets a proper OpenAPI schema.

   *Rationale:* These are the minimal features needed. `chrono`'s `serde` feature is mandatory because `DateTime` does not implement `Serialize`/`Deserialize` without it. `utoipa`'s `macros` feature (default) provides `ToSchema`; the `chrono` feature ensures `DateTime<Utc>` maps to an OpenAPI `string` with `format: date-time`.

2. **Add dependencies to anvilml-core Cargo.toml.** In `crates/anvilml-core/Cargo.toml`, add:
   - `chrono = { workspace = true }`
   - `utoipa = { workspace = true }`

   *Rationale:* Follows the established pattern where config.rs and config_load.rs use `serde = { workspace = true }` and `serde_json = { workspace = true }`.

3. **Create `crates/anvilml-core/src/types/mod.rs`.** This file declares the `job` submodule and re-exports all types from it:
   ```rust
   //! Domain types for job management.
   //!
   //! Contains `Job`, `JobStatus`, `JobSettings`, `SubmitJobRequest`, and `SubmitJobResponse`.

   pub mod job;

   pub use job::{Job, JobStatus, JobSettings, SubmitJobRequest, SubmitJobResponse};
   ```

4. **Create `crates/anvilml-core/src/types/job.rs`.** Implement all five types per the design doc §5.3 and task context:

   a. **`JobStatus` enum** — derives `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema`. Variants: `Queued`, `Running`, `Completed`, `Failed`, `Cancelled`. Derive `Copy` because it contains no heap data and is used in hot paths (job queue, scheduler).

   b. **`JobSettings` struct** — derives `Debug, Clone, Serialize, Deserialize, ToSchema, Default`. Single field: `device_preference: Option<String>` with doc comment "Requested device. `None` = auto-select by VRAM." Default impl sets `device_preference: None`.

   c. **`SubmitJobRequest` struct** — derives `Debug, Clone, Serialize, Deserialize, ToSchema, Default`. Fields: `graph: serde_json::Value` (doc: "Submitted graph JSON; opaque to Rust."), `settings: JobSettings`. Default impl sets `graph: serde_json::Value::Null`.

   d. **`SubmitJobResponse` struct** — derives `Debug, Clone, Serialize, Deserialize, ToSchema, Default`. Fields: `job_id: Uuid`, `queue_position: u32`. Default impl sets `job_id: Uuid::default()`, `queue_position: 0`.

   e. **`Job` struct** — derives `Debug, Clone, Serialize, Deserialize, ToSchema`. All fields from the design doc §5.3: `id: Uuid`, `status: JobStatus`, `graph: serde_json::Value`, `settings: JobSettings`, `created_at: DateTime<Utc>`, `started_at: Option<DateTime<Utc>>`, `completed_at: Option<DateTime<Utc>>`, `worker_id: Option<String>`, `error: Option<String>`, `queue_position: Option<u32>`. Each field gets a `///` doc comment. No `Default` derive — `created_at` requires an actual timestamp, so `Default` is not appropriate for this struct.

   *Rationale:* `Job` intentionally omits `Default` because a freshly created job always has a non-zero `created_at` timestamp set by the scheduler at submission time. Forcing `Default` on `Job` would produce a meaningless timestamp of `1970-01-01T00:00:00Z`.

5. **Modify `lib.rs`.** Add `pub mod types;` after the existing `pub mod` declarations and add a `pub use types::{...};` re-export line. The file must stay under 80 lines.

6. **Create `crates/anvilml-core/tests/job_tests.rs`.** Write ≥ 3 integration tests:
   - `test_job_json_roundtrip`: Construct a `Job` with all fields populated, serialise to JSON, deserialize back, assert equality.
   - `test_job_settings_default`: Assert `JobSettings::default().device_preference` is `None`.
   - `test_job_status_variants`: Assert all five `JobStatus` variants roundtrip through JSON (serialize each variant, deserialize, assert equality).

   Each test gets a `///` doc comment per the test documentation obligation (§11.4 of ENVIRONMENT.md).

## Public API Surface

| Item | Type | Module Path |
|------|------|-------------|
| `Job` | struct | `anvilml_core::types::Job` |
| `JobStatus` | enum | `anvilml_core::types::JobStatus` |
| `JobSettings` | struct | `anvilml_core::types::JobSettings` |
| `SubmitJobRequest` | struct | `anvilml_core::types::SubmitJobRequest` |
| `SubmitJobResponse` | struct | `anvilml_core::types::SubmitJobResponse` |

**Full struct definitions:**

```rust
// crates/anvilml-core/src/types/job.rs

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub enum JobStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Default)]
pub struct JobSettings {
    /// Requested device. None = auto-select by VRAM.
    pub device_preference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Default)]
pub struct SubmitJobRequest {
    /// Submitted graph JSON; opaque to Rust.
    pub graph: serde_json::Value,
    pub settings: JobSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, Default)]
pub struct SubmitJobResponse {
    pub job_id: Uuid,
    pub queue_position: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct Job {
    pub id: Uuid,
    pub status: JobStatus,
    pub graph: serde_json::Value,   // submitted graph JSON; opaque to Rust
    pub settings: JobSettings,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub worker_id: Option<String>,
    pub error: Option<String>,
    pub queue_position: Option<u32>,
}
```

**Module-level re-exports (lib.rs):**
```rust
pub mod types;
pub use types::{Job, JobStatus, JobSettings, SubmitJobRequest, SubmitJobResponse};
```

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/types/mod.rs` | Types module declaration and re-exports |
| CREATE | `crates/anvilml-core/src/types/job.rs` | Job, JobStatus, JobSettings, SubmitJobRequest, SubmitJobResponse |
| MODIFY | `crates/anvilml-core/src/lib.rs` | Add `pub mod types;` and `pub use types::{...};` |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Add `chrono` and `utoipa` dependencies |
| MODIFY | `Cargo.toml` | Add `chrono` and `utoipa` to `[workspace.dependencies]` |
| CREATE | `crates/anvilml-core/tests/job_tests.rs` | ≥ 3 integration tests for job types |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-core/tests/job_tests.rs` | `test_job_json_roundtrip` | A fully-populated `Job` serialises to JSON and deserialises back to an identical value, including `Option` fields (some `Some`, some `None`) and nested `JobSettings`. | None | `Job` with all fields set (id=uuid4, status=Running, graph={"nodes":[]}, settings=JobSettings{device_preference:Some("cuda")}, created_at=Utc::now(), started_at=Some(some_time), completed_at=None, worker_id=Some("worker-0"), error=None, queue_position=Some(1)) | Deserialised `Job` equals original | `cargo test -p anvilml-core -- types::job` exits 0 |
| `crates/anvilml-core/tests/job_tests.rs` | `test_job_settings_default` | `JobSettings::default()` produces `device_preference: None`, matching the documented convention that `None` means auto-select by VRAM. | None | `JobSettings::default()` | `device_preference == None` | `cargo test -p anvilml-core -- types::job` exits 0 |
| `crates/anvilml-core/tests/job_tests.rs` | `test_job_status_variants` | All five `JobStatus` enum variants roundtrip through JSON serialisation without data loss. | None | Each of: `Queued`, `Running`, `Completed`, `Failed`, `Cancelled` | Each variant serialises to its snake_case string and deserialises back to the same variant | `cargo test -p anvilml-core -- types::job` exits 0 |

## CI Impact

No CI changes required. The new tests are picked up automatically by `cargo test --workspace --features mock-hardware` (the rust-linux and rust-windows CI jobs). The new dependencies (`chrono`, `utoipa`) are standard crates that compile on both Linux and Windows without platform-specific code.

## Platform Considerations

None identified. The `DateTime<Utc>` type from chrono is timezone-naive and platform-independent. `serde_json::Value` serialisation is deterministic across platforms. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `utoipa` 5.5.0 `ToSchema` derive macro may require the `utoipa-gen` crate as a transitive dependency with specific version constraints. If the workspace resolver picks an incompatible `utoipa-gen` version, compilation fails. | Low | High | The `macros` feature on utoipa pulls in `utoipa-gen` as a dependency. Verify at ACT time that `cargo check -p anvilml-core` compiles before writing tests. If the version mismatch occurs, the ACT agent should pin `utoipa-gen` explicitly or use the latest version available via crates.io. |
| `chrono` 0.4.45's `DateTime<Utc>` with `serde` feature serialises to ISO 8601 strings (e.g. `"2026-06-14T15:50:00Z"`). If downstream code expects a different format (unix timestamp, RFC 3339 without timezone), the roundtrip will produce the chrono format which may differ from expectations. | Low | Medium | The design doc §5.3 uses `DateTime<Utc>` directly with standard serde derives, which produces ISO 8601 format. This is the correct format per the design doc. If a different format is needed, it would be a design doc deviation — flag at ACT time. |
| Adding `chrono` and `utoipa` as new workspace dependencies increases compile time and binary size for `anvilml-core`. Since `anvilml-core` is a leaf crate (no downstream crates depend on it yet), this is acceptable but worth noting. | Low | Low | `anvilml-core` is the foundation crate — all other crates will import types from it. Adding these dependencies now prevents a future refactor when the first consumer needs them. The compile-time impact is negligible for a crate with ~5 types. |
| Task context references `utoipa` with the `axum` feature, but utoipa 5.5.0 has `axum_extras` instead. If the ACT agent blindly adds `features = ["axum"]`, compilation will fail with an unknown feature error. | Medium | High | The plan explicitly notes this discrepancy and recommends `macros` + `chrono` features. The ACT agent must confirm the correct feature names at session start via crates.io API lookup before writing any Cargo.toml entry. |

## Acceptance Criteria

- [ ] `cargo check -p anvilml-core --features mock-hardware` exits 0
- [ ] `cargo test -p anvilml-core -- types::job` exits 0
- [ ] `grep -c "^#\[test\]" crates/anvilml-core/tests/job_tests.rs` returns ≥ 3
- [ ] `head -1 crates/anvilml-core/src/types/job.rs` starts with `//!` (module-level doc comment)
- [ ] `grep "^pub mod types" crates/anvilml-core/src/lib.rs` finds exactly 1 match
- [ ] `grep "^pub use types::" crates/anvilml-core/src/lib.rs` contains all five type names: `Job`, `JobStatus`, `JobSettings`, `SubmitJobRequest`, `SubmitJobResponse`
