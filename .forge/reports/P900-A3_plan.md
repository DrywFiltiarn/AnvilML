# Plan Report: P900-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P900-A3                                     |
| Phase       | 900 — Spec-Drift & Logging Retrofit         |
| Description | anvilml-core: add missing ToSchema to Job/JobStatus/JobSettings |
| Depends on  | P900-A2, P3-A1                              |
| Project     | anvilml                                     |
| Planned at  | 2026-06-30T12:58:00Z                        |
| Attempt     | 1                                           |

## Objective

Add the `ToSchema` derive to `Job`, `JobStatus`, and `JobSettings` in `anvilml-core`'s `types/job.rs`, closing the gap between `ANVILML_DESIGN.md §5.3` (which specifies these three types derive `ToSchema`) and the live code (which omits it entirely). This is a pure derive addition with zero behaviour change: no fields, variants, or serde attributes are modified. After the change, `cargo doc -p anvilml-core --no-deps` compiles successfully, confirming the derive is well-formed, and all existing serde roundtrip tests pass unchanged.

## Scope

### In Scope
- Add `use utoipa::ToSchema;` import to `crates/anvilml-core/src/types/job.rs`.
- Append `, ToSchema` to the `#[derive(...)]` attribute on `JobStatus` (line 9).
- Append `, ToSchema` to the `#[derive(...)]` attribute on `JobSettings` (line 25).
- Append `, ToSchema` to the `#[derive(...)]` attribute on `Job` (line 38).
- All three types retain their existing derives unchanged otherwise.
- Existing tests in `crates/anvilml-core/tests/job_tests.rs` remain unmodified and pass.

### Out of Scope
None. `defers_to (from JSON): absent` — this task has no deferrals and implements its full scope.

## Existing Codebase Assessment

The `job.rs` module at `crates/anvilml-core/src/types/job.rs` defines three public types: `JobStatus` (enum, 5 variants), `JobSettings` (struct, 1 field), and `Job` (struct, 9 fields). All three already derive `Debug`, `Clone`, `Serialize`, and `Deserialize`. `JobStatus` additionally derives `Copy`, `PartialEq`, and `Eq`. Each field and variant has a `///` doc comment, and the types are well-documented.

The `utoipa` crate (version 5.5.0) is already a dependency of `anvilml-core` with the `uuid` and `chrono` features enabled, and the `macros` feature (default) is available — providing the `ToSchema` derive macro. Five other type modules in the same crate (`artifact.rs`, `hardware.rs`, `events.rs`, `worker.rs`, `node.rs`) already follow the exact same pattern: `use utoipa::ToSchema;` at the top, then `, ToSchema` appended to each derive list. The `model.rs` module (P900-A4's scope) is the only other one missing it.

The design doc (§5.3) specifies the exact derive list for all three types — `Job`, `JobStatus`, and `JobSettings` each include `ToSchema` — confirming this is an isolated omission from P3-A1's context, not a systemic pattern. The existing test file `job_tests.rs` exercises serde roundtrips for all three types and will continue to pass without modification.

## Resolved Dependencies

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| crate  | utoipa  | 5.5.0           | rust-docs MCP  | macros (default, already in use) |

The `utoipa` dependency is already declared in `anvilml-core/Cargo.toml` at version 5.5.0 with features `["uuid", "chrono"]`. The `ToSchema` derive is part of the `macros` feature, which is a default feature and already enabled. No new dependency or feature flag is introduced by this task.

## Approach

1. **Add the utoipa import.** In `crates/anvilml-core/src/types/job.rs`, after the existing `use chrono::{DateTime, Utc};` / `use serde::{Deserialize, Serialize};` / `use uuid::Uuid;` imports (lines 1–3), add:
   ```rust
   use utoipa::ToSchema;
   ```
   This follows the established pattern in `artifact.rs`, `hardware.rs`, `events.rs`, `worker.rs`, and `node.rs`.

2. **Append `ToSchema` to `JobStatus`'s derive.** On line 9, change:
   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
   ```
   to:
   ```rust
   #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
   ```
   `JobStatus` keeps `Copy`, `PartialEq`, `Eq` — `ToSchema` is compatible with all of these.

3. **Append `ToSchema` to `JobSettings`'s derive.** On line 25, change:
   ```rust
   #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
   ```
   to:
   ```rust
   #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
   ```

4. **Append `ToSchema` to `Job`'s derive.** On line 38, change:
   ```rust
   #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
   ```
   to:
   ```rust
   #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
   ```

5. **Verify compilation.** Run `cargo doc -p anvilml-core --no-deps` to confirm the derive macro resolves and the documentation builds without error.

6. **Verify existing tests pass.** Run `cargo test -p anvilml-core --test job_tests` to confirm all existing serde roundtrip tests still pass. No test changes are needed because `ToSchema` is a compile-time derive with zero runtime behaviour.

Rationale: This is an additive-derive-only change. The `ToSchema` macro generates an OpenAPI schema at compile time and has no runtime effect, so the existing serde tests are sufficient acceptance criteria. The established pattern across five other type modules confirms the import placement and derive-append approach is correct.

## Public API Surface

No new public items are introduced. The following existing pub items gain a new trait impl (the `utoipa::ToSchema` trait):

| Item | Crate/Module | Before | After |
|------|-------------|--------|-------|
| `pub enum JobStatus` | `anvilml-core::types::job` | `#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]` | `#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]` |
| `pub struct JobSettings` | `anvilml-core::types::job` | `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]` | `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]` |
| `pub struct Job` | `anvilml-core::types::job` | `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]` | `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]` |

No function signatures, struct fields, enum variants, or pub re-exports change.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/types/job.rs` | Add `use utoipa::ToSchema;` import; append `, ToSchema` to derive list on `JobStatus`, `JobSettings`, and `Job` |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `crates/anvilml-core/tests/job_tests.rs` | `test_job_serde_roundtrip` | Existing full-field `Job` serde roundtrip still passes | `cargo test -p anvilml-core --test job_tests test_job_serde_roundtrip` exits 0 |
| `crates/anvilml-core/tests/job_tests.rs` | `test_job_status_all_variants_roundtrip` | All 5 `JobStatus` variants still serde roundtrip correctly | `cargo test -p anvilml-core --test job_tests test_job_status_all_variants_roundtrip` exits 0 |
| `crates/anvilml-core/tests/job_tests.rs` | `test_job_settings_default` | `JobSettings` with `None` field still roundtrips | `cargo test -p anvilml-core --test job_tests test_job_settings_default` exits 0 |
| `crates/anvilml-core/tests/job_tests.rs` | `test_job_with_nulls_roundtrip` | `Job` with all `Option` fields `None` still roundtrips | `cargo test -p anvilml-core --test job_tests test_job_with_nulls_roundtrip` exits 0 |

Note: These are existing tests, not new ones. The `ToSchema` derive has zero runtime effect, so no new test is needed. The acceptance criterion is that these existing tests continue to pass.

## CI Impact

No CI changes required. The change is a derive-only addition with no new file types, no new test modules, no new dependencies, and no observable behaviour change. The existing CI jobs (`rust-linux`, `rust-windows`) will pick up the change automatically through `cargo test --workspace --features mock-hardware` and `cargo clippy --workspace --features mock-hardware -- -D warnings`.

## Platform Considerations

None identified. The `utoipa::ToSchema` derive is platform-neutral — it generates compile-time OpenAPI schema metadata with no platform-specific code paths. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `ToSchema` derive macro fails to resolve if `utoipa`'s `macros` feature is not enabled in the transitive dependency graph | Low | High — compile failure | Verified via rust-docs MCP: `utoipa` 5.5.0 has `macros` as a default feature, and `anvilml-core/Cargo.toml` already declares `utoipa = { version = "5.5.0", features = ["uuid", "chrono"] }` — the default features (including `macros`) are enabled unless explicitly disabled with `default-features = false`, which they are not. |
| Adding `ToSchema` to `JobStatus`'s derive list could conflict with an existing trait bound (e.g. if `Copy` or `PartialEq` were incompatible) | Low | Medium — compile failure | Verified: `ToSchema` requires only `Debug + Clone + Serialize + Deserialize` (plus optional `PartialEq` for `PartialEq`-based schemas). `JobStatus` already derives all four plus `Copy` and `Eq`, so no conflict is possible. |
| The existing tests in `job_tests.rs` don't exercise `ToSchema` specifically, so a broken derive might not be caught by the test suite | Low | Low — caught by `cargo doc` acceptance criterion | The acceptance criteria include `cargo doc -p anvilml-core --no-deps` which exercises the derive macro fully. If `ToSchema` is syntactically or semantically broken, the doc build will fail. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-core --test job_tests` exits 0
- [ ] `cargo doc -p anvilml-core --no-deps` exits 0
