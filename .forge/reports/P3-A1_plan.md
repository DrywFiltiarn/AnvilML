# Plan Report: P3-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P3-A1                                         |
| Phase       | 003 — Core Domain Types                     |
| Description | anvilml-core: AnvilError enum and error model |
| Depends on  | P2-A5                                         |
| Project     | anvilml                                       |
| Planned at  | 2026-06-01T09:16:43Z                         |
| Attempt     | 1                                             |

## Objective

Create the centralized `AnvilError` error type for `anvilml-core`, replacing the ad-hoc manual error handling currently used in `config_load.rs`. This enum uses `thiserror` to derive `Display`, `std::error::Error`, and `From<std::io::Error>` automatically. The type must be `Send + Sync` so it can cross async boundaries (e.g., through `axum`'s `ErrorResponses`). All variants specified in the task — `ConfigLoad`, `Io`, `Json`, `InvalidGraph`, `WorkerDead`, `JobNotFound`, `ArtifactNotFound`, `DbError`, `PayloadTooLarge` — are included with descriptive error messages. The type is re-exported from `lib.rs` and unit-tested to ensure all variants produce valid error strings, implement `Send + Sync`, and compile cleanly.

## Scope

### In Scope
- Add `thiserror = "1"` dependency to `crates/anvilml-core/Cargo.toml`
- Create `src/error.rs` with the `AnvilError` enum
- Implement `#[error("...")]` Display messages for every variant
- Derive `Debug`, `Clone`, `Send + Sync` (via `#[derive(Debug, Clone)]` + `unsafe impl Send + Sync` or by choosing Send+Sync-safe variants)
- Implement `From<std::io::Error>` via `#[from]` on the `Io` variant
- Re-export `AnvilError` from `src/lib.rs`
- Write unit tests in `src/error.rs` under `#[cfg(test)]`
- Verify `cargo test -p anvilml-core -- error` exits 0

### Out of Scope
- Refactoring or replacing `ConfigError` in `config_load.rs` (retained as-is for Phase 3; P3-A1 only introduces the new type)
- Creating any other domain types (`Job`, `Model`, `Artifact`, `HardwareInfo`, etc.) — those belong to P3-A2 through P3-A5
- Adding utoipa/ToSchema derives (those are added in P3-A2)
- Writing integration tests or server-side error mapping
- CI workflow changes

## Approach

1. **Add thiserror dependency.** Append `thiserror = "1"` to `[dependencies]` in `crates/anvilml-core/Cargo.toml`. Use the latest stable 1.x version.

2. **Create `src/error.rs`.** Define the `AnvilError` enum with all nine variants:
   - `ConfigLoad(String)` — for TOML/file loading failures
   - `Io(#[from] std::io::Error)` — generic I/O errors, auto-From
   - `Json(String)` — serde JSON serialization/deserialization failures
   - `InvalidGraph(String)` — DAG validation failures
   - `WorkerDead(String)` — worker process death notifications
   - `JobNotFound(Uuid)` — missing job by UUID
   - `ArtifactNotFound(String)` — missing artifact by identifier
   - `DbError(String)` — SQLite/database errors
   - `PayloadTooLarge { size_mib: u32, limit_mib: u32 }` — payload size enforcement

3. **Derive and annotate.** Each variant gets a `#[error("...")]` attribute with a human-readable message. Derive `Debug, Clone`. Ensure all variant data is `Send + Sync` (`String`, `u32`, `Uuid` from the uuid crate's types — but since uuid is not yet added to core, use `String` or a raw `[u8; 16]`; however the task spec says `Uuid`, so we must either add the `uuid` crate now or use `String`. Since P3-A2 adds uuid, and this task depends on P2-A5 but not P3-A2, the safest approach is to use `String` for JobNotFound and import uuid conditionally. Actually — re-reading the spec: "JobNotFound(Uuid)" — the task explicitly requires Uuid. The most pragmatic approach: add `uuid = { version = "1", features = ["serde"] }` to Cargo.toml alongside thiserror, since P3-A2 also needs it and having it in core first avoids dependency churn.

   Correction: The task says "Create src/error.rs: AnvilError enum variants ConfigLoad(String), Io(#[from] std::io::Error), Json(String), InvalidGraph(String), WorkerDead(String), JobNotFound(Uuid), ArtifactNotFound(String), DbError(String), PayloadTooLarge{size_mib:u32,limit_mib:u32}." — so Uuid is required. We add uuid with serde feature to Cargo.toml.

4. **Implement Send + Sync.** All variant payloads (`String`, `std::io::Error`, `Uuid`) are already `Send + Sync`, so `#[derive(Debug, Clone)]` on the enum suffices — no manual impl needed.

5. **Re-export from `lib.rs`.** Add `pub mod error;` and `pub use error::AnvilError;` to `src/lib.rs`.

6. **Write unit tests.** In `src/error.rs`, under `#[cfg(test)]`, add:
   - Test that each variant's `to_string()` produces a non-empty message
   - Test that `Io` variant auto-converts from `std::io::Error` via `From`
   - Test compile-time `Send + Sync` bounds (static assertions)
   - Test `PayloadTooLarge` includes both size and limit in the error string

7. **Verify with cargo test.** Run `cargo test -p anvilml-core -- error` and confirm exit code 0.

## Files Affected

| Action   | Path                                      | Description                                              |
|----------|-------------------------------------------|----------------------------------------------------------|
| MODIFY   | crates/anvilml-core/Cargo.toml            | Add `thiserror` and `uuid` dependencies                  |
| CREATE   | crates/anvilml-core/src/error.rs          | AnvilError enum with all variants, Display, tests        |
| MODIFY   | crates/anvilml-core/src/lib.rs            | Add `pub mod error;` and `pub use error::AnvilError;`    |

## Tests

| Test ID / Name            | File                              | Validates                                         |
|---------------------------|-----------------------------------|----------------------------------------------------|
| `error_display_all_variants` | crates/anvilml-core/src/error.rs  | Every variant produces a non-empty Display string  |
| `error_io_from_std_io_error` | crates/anvilml-core/src/error.rs  | `AnvilError::from(std::io::Error)` creates Io variant |
| `error_send_sync_bounds`    | crates/anvilml-core/src/error.rs  | `AnvilError` satisfies `Send + Sync` at compile time |
| `error_payload_too_large_display` | crates/anvilml-core/src/error.rs | Error message includes both size_mib and limit_mib  |

## CI Impact

No CI changes required. This task only adds a dependency and new source files to `anvilml-core`. The existing CI workflow (`cargo test -p anvilml-core`) will automatically pick up the new tests.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| Adding `uuid` crate to core before P3-A2 creates a dependency ordering concern | Low | Low | uuid is needed by the spec; adding it here is cleaner than adding it again in P3-A2. P3-A2 will simply use the existing dep. |
| Manual `Send + Sync` impl breaks with future variant additions | Low | Medium | All planned payloads (`String`, `std::io::Error`, `Uuid`) are inherently `Send + Sync`. No manual impl needed — derive suffices. |
| Conflict with existing `ConfigError` in `config_load.rs` | Low | Low | `ConfigError` is retained as-is. `AnvilError` is a new, broader error type for future use. They coexist without conflict. |

## Acceptance Criteria

- [ ] `crates/anvilml-core/src/error.rs` exists with the `AnvilError` enum and all 9 variants
- [ ] Each variant has a `#[error("...")]` message that produces non-empty output via `.to_string()`
- [ ] `Io(#[from] std::io::Error)` enables automatic conversion from `std::io::Error`
- [ ] `AnvilError` implements `Send + Sync` (verified by static type assertions in tests)
- [ ] `uuid` and `thiserror` appear in `crates/anvilml-core/Cargo.toml`
- [ ] `pub mod error;` and `pub use error::AnvilError;` present in `src/lib.rs`
- [ ] `cargo test -p anvilml-core -- error` exits with code 0
