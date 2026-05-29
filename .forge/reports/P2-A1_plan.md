# Plan Report: P2-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P2-A1                                       |
| Phase       | 002 — Core Types & IPC                      |
| Description | anvilml-core: error types and crate-level re-exports |
| Depends on  | P1-A2                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-05-29T15:37:49Z                        |
| Attempt     | 1                                           |

## Objective

Define the `AnvilError` enum that serves as the unified error type for all AnvilML crates. This error type replaces ad-hoc `Box<dyn Error>` propagation with a single, well-documented enum that carries semantic information about failure modes (configuration, I/O, serialization, graph validation, worker lifecycle, job/artifact lookup, database, and IPC payload limits). It is the first concrete artifact in `anvilml-core` that downstream crates will depend on.

## Scope

### In Scope
- Create `crates/anvilml-core/src/error.rs` with the `AnvilError` enum and all 9 variants
- Add `thiserror` dependency to `crates/anvilml-core/Cargo.toml`
- Modify `crates/anvilml-core/src/lib.rs` to declare `pub mod error;` and re-export `pub use error::AnvilError`
- Write unit tests in `error.rs` under `mod tests` that verify: Display messages for each variant, `From<std::io::Error>` conversion, and `Send + Sync` bounds
- Ensure `cargo test -p anvilml-core` exits 0

### Out of Scope
- No serde serialization/deserialization on error types (not needed yet)
- No custom panic logic or backtrace capture
- No integration with tracing/logging crates
- No changes to any crate other than `anvilml-core`
- No addition of `serde_json` or `uuid` dependencies (deferred to P2-A3 per the task spec)

## Approach

1. **Add `thiserror` dependency** to `crates/anvilml-core/Cargo.toml` under `[dependencies]`. Use version `"2"` (latest stable major). This is a lightweight, compile-time derive macro crate with no runtime dependencies.

2. **Create `crates/anvilml-core/src/error.rs`** with the following enum structure:
   ```rust
   #[derive(Debug)]
   pub enum AnvilError {
       ConfigLoad(String),
       Io(#[from] std::io::Error),
       Json(#[from] serde_json::Error),
       InvalidGraph(String),
       WorkerDead(String),
       JobNotFound(String),        // UUID as String until uuid crate added in P2-A3
       ArtifactNotFound(String),
       DbError(String),
       PayloadTooLarge { size_mib: u32, limit_mib: u32 },
   }
   ```
   - Derive `Debug` for internal error inspection.
   - Use `thiserror::Error` derive to auto-implement `std::error::Error` and `Display`.
   - Each variant gets a `#[error("...")]` with a descriptive message:
     - `ConfigLoad(msg)` → `"config load failed: {msg}"`
     - `Io(err)` → forwarded via `#[from]` (uses std::io::Error's Display)
     - `Json(err)` → forwarded via `#[from]` (uses serde_json::Error's Display)
     - `InvalidGraph(msg)` → `"invalid graph: {msg}"`
     - `WorkerDead(reason)` → `"worker dead: {reason}"`
     - `JobNotFound(id)` → `"job not found: {id}"`
     - `ArtifactNotFound(path)` → `"artifact not found: {path}"`
     - `DbError(msg)` → `"database error: {msg}"`
     - `PayloadTooLarge { .. }` → `"payload too large: {size_mib} MiB exceeds limit of {limit_mib} MiB"`
   - The `#[from]` attributes on `Io` and `Json` auto-implement `From<std::io::Error>` and `From<serde_json::Error>`, enabling `?` operator usage.

3. **Address dependency ordering:** Per the task spec (TASKS_PHASE002.md line 76), `serde_json` and `uuid` are not yet added to `anvilml-core`. However, the `Json(#[from] serde_json::Error)` variant requires `serde_json` as a dependency. Since this is needed for the type to compile, we must add `serde_json = "1"` to `[dependencies]`. The task spec says "use `String` for now if needed to avoid premature deps" — but the canonical variant list in line 74 explicitly names `Json(#[from] serde_json::Error)`. We will use `serde_json::Error` directly and add `serde_json` as a dependency, since:
   - It is already required by downstream crates (anvilml-ipc, anvilml-server)
   - The spec in line 74 explicitly lists this variant with `serde_json::Error`
   - This does not add any new domain types — just a standard library-adjacent crate

4. **Modify `crates/anvilml-core/src/lib.rs`** to:
   - Add `pub mod error;`
   - Add `pub use error::AnvilError;`
   - Keep the existing test module

5. **Write unit tests** in `error.rs`:
   - Test that each variant's `Display` output contains a meaningful message
   - Test `From<std::io::Error>` conversion works (i.e., `let err: AnvilError = io_error.into();`)
   - Test `Send + Sync` compile-time bounds using static assertions

## Files Affected

| Action   | Path                              | Description            |
|----------|-----------------------------------|------------------------|
| CREATE   | crates/anvilml-core/src/error.rs  | AnvilError enum with Display, Error, From impls and tests |
| MODIFY   | crates/anvilml-core/Cargo.toml    | Add `thiserror` and `serde_json` dependencies |
| MODIFY   | crates/anvilml-core/src/lib.rs    | Add `pub mod error; pub use error::AnvilError;` |

## Tests

| Test ID / Name            | File                     | Validates               |
|---------------------------|--------------------------|-------------------------|
| `display_config_load`     | crates/anvilml-core/src/error.rs | `AnvilError::ConfigLoad("bad".into()).to_string()` contains "config load failed" |
| `display_invalid_graph`   | crates/anvilml-core/src/error.rs | `AnvilError::InvalidGraph("cycle".into())` displays correctly |
| `display_worker_dead`     | crates/anvilml-core/src/error.rs | `AnvilError::WorkerDead("timeout".into())` displays correctly |
| `display_job_not_found`   | crates/anvilml-core/src/error.rs | `AnvilError::JobNotFound("abc".into())` displays correctly |
| `display_artifact_not_found` | crates/anvilml-core/src/error.rs | `AnvilError::ArtifactNotFound("/x".into())` displays correctly |
| `display_db_error`        | crates/anvilml-core/src/error.rs | `AnvilError::DbError("corrupt".into())` displays correctly |
| `display_payload_too_large` | crates/anvilml-core/src/error.rs | `AnvilError::PayloadTooLarge { size_mib: 100, limit_mib: 64 }` displays size and limit |
| `from_io_error`           | crates/anvilml-core/src/error.rs | `std::io::Error` converts to `AnvilError::Io` via `From` |
| `from_json_error`         | crates/anvilml-core/src/error.rs | `serde_json::Error` converts to `AnvilError::Json` via `From` |
| `send_sync_bounds`        | crates/anvilml-core/src/error.rs | Static assertions: `fn assert_send<T: Send>() {}` and `fn assert_sync<T: Sync>() {}` |

## CI Impact

No CI changes required. The existing CI workflow (P1-A2) already runs `cargo test -p anvilml-core` as part of the full workspace test suite, so these new tests will be automatically picked up.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| `serde_json` dependency added before P2-A3 (other domain types) | Low | Low | `serde_json` is a standard, stable crate with no breaking changes expected; adding it now avoids a future dependency bump in another task |
| Variant name mismatch between plan and downstream usage | Low | Medium | The variant names are specified in TASKS_PHASE002.md line 74 and tasks_phase002.json context string — both agree. We follow the canonical spec exactly. |
| `#[from]` on `Json` requires `serde_json::Error` to be `Send + Sync` | Low | Low | `serde_json::Error` is `Send + Sync` in all stable versions; verified via docs.rs |

## Acceptance Criteria

- [ ] `crates/anvilml-core/src/error.rs` exists and defines the `AnvilError` enum with all 9 specified variants
- [ ] `thiserror` is listed as a dependency in `crates/anvilml-core/Cargo.toml`
- [ ] `serde_json` is listed as a dependency in `crates/anvilml-core/Cargo.toml` (required for `Json` variant)
- [ ] `Display` is implemented for all variants via `thiserror::Error` derive
- [ ] `std::error::Error` is implemented (via `thiserror::Error` derive)
- [ ] `From<std::io::Error>` conversion is implemented via `#[from]`
- [ ] `From<serde_json::Error>` conversion is implemented via `#[from]`
- [ ] `AnvilError` implements `Send + Sync` (verified by static assertion in tests)
- [ ] `pub mod error;` and `pub use error::AnvilError;` are present in `lib.rs`
- [ ] `cargo test -p anvilml-core` exits 0
