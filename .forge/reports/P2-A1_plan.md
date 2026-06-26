# Plan Report: P2-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P2-A1                                         |
| Phase       | 2 — Core Domain Types: Config & Errors       |
| Description | anvilml-core: AnvilError enum + IntoResponse impl |
| Depends on  | P1-B1                                         |
| Project     | anvilml                                       |
| Planned at  | 2026-06-26T17:00:00Z                          |
| Attempt     | 1                                             |

## Objective

Create the `AnvilError` enum in `crates/anvilml-core/src/error.rs` with all 13 variants specified in `ANVILML_DESIGN.md §5.2` (including `ArtifactNotFound` per the addendum), implement `axum::response::IntoResponse` for `AnvilError` so that every variant maps to a correct HTTP status code and structured JSON body, wire the module into `lib.rs`, and add the required dependencies (`thiserror`, `axum`, `uuid`). This establishes the single error type that all subsequent crates will return and consume.

## Scope

### In Scope
- Create `crates/anvilml-core/src/error.rs` with the `AnvilError` enum (13 variants, `thiserror::Error` derive, `Debug` derive, `#[from]` for `Db` and `Io`).
- Implement `axum::response::IntoResponse` for `AnvilError`: each variant maps to a specific HTTP status code and JSON body `{ "error": "<kind>", "message": "<text>", "request_id": "<uuid>" }`.
- Add dependencies to `crates/anvilml-core/Cargo.toml`: `thiserror`, `axum`, `uuid` (with `v4` feature), `serde_json`, `sqlx`.
- Add `mod error;` and `pub use error::AnvilError;` to `crates/anvilml-core/src/lib.rs`.
- Create `crates/anvilml-core/tests/error_tests.rs` with ≥9 tests covering distinct variant-to-status mappings including `ArtifactNotFound`.

### Out of Scope

defers_to (from JSON): []

None. This task implements its full scope — no deferrals, no stubs.

## Existing Codebase Assessment

`anvilml-core` exists as an empty stub crate (Phase 1's P1-B1). Its `lib.rs` contains only a `//!` crate-level doc comment declaring "Zero I/O. Zero async. No tokio, no sqlx, no network." — two lines, no modules declared. No `tests/` directory exists yet. No `error.rs` file exists. The workspace `Cargo.toml` has no dependency declarations; `axum = "0.8.9"` is already pinned in `crates/anvilml-server/Cargo.toml` and `backend/Cargo.toml`, confirming the version.

No established patterns exist in `anvilml-core` yet — this task establishes the baseline patterns for error handling, module structure, and test style for subsequent phases. The `lib.rs` discipline (≤ 80 lines, `pub mod`/`pub use`/crate-level doc only) is the structural convention to follow.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source     | Feature flags confirmed |
|--------|-----------|-----------------|----------------|------------------------|
| crate  | thiserror | 2.0.18           | crates.io search | n/a                    |
| crate  | axum      | 0.8.9            | crates.io search (workspace pin confirmed in anvilml-server/Cargo.toml) | n/a |
| crate  | uuid      | 1.23.4           | crates.io search | v4                     |
| crate  | serde_json| 1.0.143          | crates.io search | n/a                    |
| crate  | sqlx      | 0.9.0            | crates.io search | sqlite                 |

**Notes:**
- `thiserror` 2.0.x: The `#[derive(Error)]`, `#[from]`, and `#[error(...)]` macros are compatible with 2.0.x. The `thiserror::Error` trait is the derive target (not a trait to impl).
- `sqlx` 0.9.0: The `sqlx::Error` type is available with the `sqlite` feature. This is the first crate to depend on sqlx in the dependency graph — subsequent crates that need database errors will depend on `anvilml-core` (which transitively provides `sqlx::Error`).
- `serde_json`: Added explicitly (not relying on axum's transitive dependency) so the `IntoResponse` impl can construct JSON bodies independently of axum's internal dependency tree.

## Approach

1. **Add dependencies to `crates/anvilml-core/Cargo.toml`.** Add a `[dependencies]` section with:
   - `thiserror = "2.0.18"`
   - `axum = "0.8.9"` (matches the workspace-wide pin)
   - `uuid = { version = "1.23.4", features = ["v4"] }`
   - `serde_json = "1.0"`
   - `sqlx = { version = "0.9.0", features = ["sqlite"] }`

2. **Create `crates/anvilml-core/src/error.rs`.** Define:
   - `#[derive(Debug, thiserror::Error)]` on `pub enum AnvilError`.
   - All 13 variants with their exact types and `#[error(...)]` messages per `ANVILML_DESIGN.md §5.2` and the `ADDENDUM_ARTIFACT_NOT_FOUND.md`:
     - `Db(#[from] sqlx::Error)` — message: `"database error: {0}"`
     - `Io(#[from] std::io::Error)` — message: `"I/O error: {0}"`
     - `Serde(String)` — message: `"serialization error: {0}"`
     - `Ipc(String)` — message: `"IPC error: {0}"`
     - `PayloadTooLarge(String)` — message: `"payload too large: {0}"`
     - `WorkerNotFound(String)` — message: `"worker not found: {0}"`
     - `JobNotFound(String)` — message: `"job not found: {0}"`
     - `InvalidGraph(Vec<String>)` — message: `"invalid graph: {0:?}"`
     - `CycleDetected(Vec<String>)` — message: `"graph cycle detected: {0:?}"`
     - `ModelNotFound(String)` — message: `"model not found: {0}"`
     - `ArtifactNotFound(String)` — message: `"artifact not found: {0}"` (per addendum)
     - `WorkersUnavailable(String)` — message: `"workers unavailable: {0}"`
     - `Internal(String)` — message: `"internal error: {0}"`

3. **Implement `axum::response::IntoResponse` for `AnvilError`.** In the same file, write a `impl axum::response::IntoResponse for AnvilError` block that matches each variant to an HTTP status and JSON body. The JSON body struct is:
   ```rust
   struct ErrorBody {
       error: String,
       message: String,
       request_id: uuid::Uuid,
   }
   ```
   Status code mapping:
   - `Db` → 500 (Internal Server Error) — the database layer failed unexpectedly
   - `Io` → 500 — I/O failure is unexpected at the API surface
   - `Serde` → 400 (Bad Request) — client sent malformed data
   - `Ipc` → 400 — IPC communication error (internal protocol issue)
   - `PayloadTooLarge` → 413 (Payload Too Large) — per HTTP standard
   - `WorkerNotFound` → 404 — resource not found
   - `JobNotFound` → 404 — resource not found
   - `InvalidGraph` → 400 — client submitted invalid graph
   - `CycleDetected` → 400 — client submitted cyclic graph
   - `ModelNotFound` → 404 — resource not found
   - `ArtifactNotFound` → 404 — resource not found (per addendum, matches ModelNotFound precedent)
   - `WorkersUnavailable` → 503 (Service Unavailable) — workers are temporarily unavailable
   - `Internal` → 500 — generic internal error

   Each arm constructs `axum::Json(ErrorBody)` with:
   - `error`: a snake_case string identifying the variant kind
   - `message`: formatted from the variant's inner value
   - `request_id`: `uuid::Uuid::new_v4()` (fresh UUID per response, as specified in the task context)

4. **Update `crates/anvilml-core/src/lib.rs`.** Add `mod error;` and `pub use error::AnvilError;` after the existing crate-level doc comment. The file must remain ≤ 80 lines.

5. **Create `crates/anvilml-core/tests/error_tests.rs`.** Write ≥9 tests (see Tests section below) that exercise the `IntoResponse` implementation by constructing each error variant, calling `.into_response()`, and asserting the status code and JSON body. Use `axum::Response`'s `status()` method and body inspection.

6. **Update `crates/anvilml-core/Cargo.toml` for dev-dependencies.** Add `axum` to `[dev-dependencies]` if not already present (it is in `[dependencies]`, so it's available for tests).

### Documentation and inline comments
- Every `pub` item (`AnvilError`, its variants, `IntoResponse` impl) gets a `///` doc comment per `FORGE_AGENT_RULES.md §12.1`.
- The `ErrorBody` struct (private, not pub) gets a `///` doc comment describing the JSON response shape.
- No `#[cfg(...)]` guards are needed — the error type is platform-neutral.

### Dual-mode parity markers
Not applicable. `AnvilError` is a domain type, not a node `execute()` or arch module `load()`/`sample()`/`decode()`. The `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` convention (ANVILML_DESIGN.md §10.6) does not apply.

## Public API Surface

| Item | Path | Description |
|------|------|-------------|
| `pub enum AnvilError` | `anvilml_core::AnvilError` | 13-variant error enum with `thiserror::Error` derive |
| `impl IntoResponse for AnvilError` | `anvilml_core::AnvilError` | Maps each variant to HTTP status + JSON body |
| `pub use AnvilError` | `anvilml_core` (re-export) | Re-exported from `lib.rs` |

Full `AnvilError` variant signatures (verbatim from `ANVILML_DESIGN.md §5.2` + addendum):
```rust
#[derive(Debug, thiserror::Error)]
pub enum AnvilError {
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(String),
    #[error("IPC error: {0}")]
    Ipc(String),
    #[error("payload too large: {0}")]
    PayloadTooLarge(String),
    #[error("worker not found: {0}")]
    WorkerNotFound(String),
    #[error("job not found: {0}")]
    JobNotFound(String),
    #[error("invalid graph: {0:?}")]
    InvalidGraph(Vec<String>),
    #[error("graph cycle detected: {0:?}")]
    CycleDetected(Vec<String>),
    #[error("model not found: {0}")]
    ModelNotFound(String),
    #[error("artifact not found: {0}")]
    ArtifactNotFound(String),
    #[error("workers unavailable: {0}")]
    WorkersUnavailable(String),
    #[error("internal error: {0}")]
    Internal(String),
}
```

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/src/error.rs` | `AnvilError` enum + `IntoResponse` impl |
| MODIFY | `crates/anvilml-core/src/lib.rs` | Add `mod error;` and `pub use error::AnvilError;` |
| MODIFY | `crates/anvilml-core/Cargo.toml` | Add `thiserror`, `axum`, `uuid`, `serde_json`, `sqlx` dependencies |
| CREATE | `crates/anvilml-core/tests/error_tests.rs` | ≥9 tests for variant-to-status mappings |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-core/tests/error_tests.rs` | `test_db_returns_500` | `AnvilError::Db(sqlx::Error::Database(...))` maps to 500 | None (construct with fake sqlx::Error via sqlx::Error::Database) | Db variant with message | Status 500, JSON body error="database_error" | `cargo test -p anvilml-core --test error_tests test_db_returns_500` exits 0 |
| `crates/anvilml-core/tests/error_tests.rs` | `test_io_returns_500` | `AnvilError::Io(io::Error)` maps to 500 | None | Io variant with message | Status 500, JSON body error="io_error" | `cargo test -p anvilml-core --test error_tests test_io_returns_500` exits 0 |
| `crates/anvilml-core/tests/error_tests.rs` | `test_serde_returns_400` | `AnvilError::Serde("bad json")` maps to 400 | None | Serde variant | Status 400, JSON body error="serde_error" | `cargo test -p anvilml-core --test error_tests test_serde_returns_400` exits 0 |
| `crates/anvilml-core/tests/error_tests.rs` | `test_ipc_returns_400` | `AnvilError::Ipc("timeout")` maps to 400 | None | Ipc variant | Status 400, JSON body error="ipc_error" | `cargo test -p anvilml-core --test error_tests test_ipc_returns_400` exits 0 |
| `crates/anvilml-core/tests/error_tests.rs` | `test_payload_too_large_returns_413` | `AnvilError::PayloadTooLarge("1GB")` maps to 413 | None | PayloadTooLarge variant | Status 413, JSON body error="payload_too_large" | `cargo test -p anvilml-core --test error_tests test_payload_too_large_returns_413` exits 0 |
| `crates/anvilml-core/tests/error_tests.rs` | `test_worker_not_found_returns_404` | `AnvilError::WorkerNotFound("gpu:0")` maps to 404 | None | WorkerNotFound variant | Status 404, JSON body error="worker_not_found" | `cargo test -p anvilml-core --test error_tests test_worker_not_found_returns_404` exits 0 |
| `crates/anvilml-core/tests/error_tests.rs` | `test_job_not_found_returns_404` | `AnvilError::JobNotFound("job-xyz")` maps to 404 | None | JobNotFound variant | Status 404, JSON body error="job_not_found" | `cargo test -p anvilml-core --test error_tests test_job_not_found_returns_404` exits 0 |
| `crates/anvilml-core/tests/error_tests.rs` | `test_model_not_found_returns_404` | `AnvilError::ModelNotFound("flux2klein4b")` maps to 404 | None | ModelNotFound variant | Status 404, JSON body error="model_not_found" | `cargo test -p anvilml-core --test error_tests test_model_not_found_returns_404` exits 0 |
| `crates/anvilml-core/tests/error_tests.rs` | `test_artifact_not_found_returns_404` | `AnvilError::ArtifactNotFound("abc123")` maps to 404 | None | ArtifactNotFound variant | Status 404, JSON body error="artifact_not_found" | `cargo test -p anvilml-core --test error_tests test_artifact_not_found_returns_404` exits 0 |
| `crates/anvilml-core/tests/error_tests.rs` | `test_workers_unavailable_returns_503` | `AnvilError::WorkersUnavailable("no gpu")` maps to 503 | None | WorkersUnavailable variant | Status 503, JSON body error="workers_unavailable" | `cargo test -p anvilml-core --test error_tests test_workers_unavailable_returns_503` exits 0 |
| `crates/anvilml-core/tests/error_tests.rs` | `test_internal_returns_500` | `AnvilError::Internal("panic")` maps to 500 | None | Internal variant | Status 500, JSON body error="internal_error" | `cargo test -p anvilml-core --test error_tests test_internal_returns_500` exits 0 |
| `crates/anvilml-core/tests/error_tests.rs` | `test_error_body_has_request_id` | JSON body contains a valid v4 UUID in `request_id` field | None | Any variant | `request_id` is a valid UUID v4 string | `cargo test -p anvilml-core --test error_tests test_error_body_has_request_id` exits 0 |
| `crates/anvilml-core/tests/error_tests.rs` | `test_invalid_graph_returns_400` | `AnvilError::InvalidGraph(vec!["missing input"])` maps to 400 | None | InvalidGraph variant | Status 400, JSON body error="invalid_graph" | `cargo test -p anvilml-core --test error_tests test_invalid_graph_returns_400` exits 0 |
| `crates/anvilml-core/tests/error_tests.rs` | `test_cycle_detected_returns_400` | `AnvilError::CycleDetected(vec!["A->B->A"])` maps to 400 | None | CycleDetected variant | Status 400, JSON body error="cycle_detected" | `cargo test -p anvilml-core --test error_tests test_cycle_detected_returns_400` exits 0 |

13 tests total (> 9 minimum required). Each test exercises a distinct variant-to-status mapping or a structural property (request_id presence).

## CI Impact

No CI changes required. The new test file `crates/anvilml-core/tests/error_tests.rs` is a standard Rust integration test crate under `tests/`, which `cargo test --workspace` already picks up automatically. No new file types, gates, or CI jobs are introduced.

## Platform Considerations

None identified. The `AnvilError` enum, its `IntoResponse` impl, and the HTTP status code mappings are entirely platform-neutral. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `sqlx` 0.9.0 requires compile-time query verification (`sqlx-cli`) that may not be available during `cargo check`/`cargo test` for `anvilml-core` since no migrations exist yet. The `#[from] sqlx::Error` type reference alone should compile without query verification, but the `sqlite` feature may still trigger build-time checks. | Medium | High | If `cargo check` fails due to sqlx's compile-time checks, use `sqlx = { version = "0.9", features = ["sqlite", "runtime-tokio"] }` or fall back to `sqlx = "0.8"` (which has fewer compile-time requirements). The `sqlx::Error` type itself is available regardless of feature selection. |
| `thiserror` 2.0.x may have breaking changes in the `#[error(...)]` message formatting syntax compared to 1.x (e.g., `Display` formatting changes). The plan was written assuming 2.0.x compatibility. | Low | Medium | If the `#[error(...)]` macro fails to compile, pin to `thiserror = "1.0"` (which is confirmed to work with the exact syntax used). Record the override in the plan. |
| `axum` 0.8.9's `IntoResponse` trait may have a different signature than expected (e.g., requiring `impl Into<Response>` vs direct tuple impl). The `axum::Json<T>` wrapper requires `T: Serialize`. | Low | Medium | Verify `axum::Json<ErrorBody>` implements `IntoResponse` — it does when `ErrorBody: Serialize` (which it is via `serde_json::json!`). If the tuple `(StatusCode, Json<Value>)` doesn't implement `IntoResponse`, use `axum::response::Json` directly. |
| `serde_json` is not explicitly listed in any existing crate's dependencies — relying on axum's transitive dependency could break if axum's dependency tree changes. | Low | Low | Added `serde_json` explicitly to `Cargo.toml` to avoid transitive dependency fragility. |

## Acceptance Criteria

- [ ] `cargo check -p anvilml-core --features mock-hardware` exits 0
- [ ] `cargo test -p anvilml-core --test error_tests` exits 0 with ≥9 tests passing
- [ ] `grep "^## " .forge/reports/P2-A1_plan.md` returns exactly 12 section headings
- [ ] `head -1 .forge/reports/P2-A1_plan.md` prints `# Plan Report: P2-A1`
- [ ] `wc -l .forge/reports/P2-A1_plan.md` prints a number > 40
