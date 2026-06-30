# Plan Report: P7-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P7-A1                                       |
| Phase       | 7 — IPC Foundations                         |
| Description | anvilml-ipc: IPC-specific error types       |
| Depends on  | P3-A11, P900-A3                             |
| Project     | anvilml                                     |
| Planned at  | 2026-06-30T19:25:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `crates/anvilml-ipc/src/error.rs` defining the `IpcError` enum with six variants (`BindFailed`, `SendFailed`, `RecvFailed`, `SerializationFailed`, `PayloadTooLarge`, `UnknownWorker`) using `thiserror::Error` derive, implement `From<IpcError> for AnvilError` mapping to the existing `AnvilError::Ipc(String)` variant, add `thiserror` to the crate's `Cargo.toml`, and wire `mod error; pub use error::IpcError;` into `lib.rs`. The deliverable is a fully testable error type that IPC operations throughout this crate will return, with a clean conversion into the domain-level `AnvilError` so callers outside this crate never need to know about `IpcError`.

## Scope

### In Scope
- Create `crates/anvilml-ipc/src/error.rs` with `IpcError` enum and `thiserror::Error` derive.
- Define six variants: `BindFailed(String)`, `SendFailed(String)`, `RecvFailed(String)`, `SerializationFailed(String)`, `PayloadTooLarge{actual: usize, max: usize}`, `UnknownWorker(String)`.
- Implement `From<IpcError> for AnvilError` mapping every variant to `AnvilError::Ipc(String)` with a descriptive message.
- Add `thiserror = "2.0.18"` to `crates/anvilml-ipc/Cargo.toml` (same version as `anvilml-core`).
- Update `crates/anvilml-ipc/src/lib.rs` with `mod error;` and `pub use error::IpcError;`.
- Create `crates/anvilml-ipc/tests/error_tests.rs` with >=5 tests covering each variant's `Display` output and the `From<IpcError> for AnvilError` conversion.

### Out of Scope
None. `defers_to (from JSON): []` — this task must implement its full scope with no deferrals.

## Existing Codebase Assessment

The `anvilml-ipc` crate already exists as a stub (Phase 1's P1-B4) with version `0.1.1`. Its `lib.rs` currently exports only `EventBroadcaster` from the `ws` module (which was created by Phase 7's P7-C1). The crate depends on `anvilml-core` (path dependency), `tokio` (sync feature), and `uuid` (dev-dependency).

`AnvilError` is defined in `crates/anvilml-core/src/error.rs` with 13 variants including the existing `Ipc(String)` variant at line 59-60, which is explicitly documented as "Internal IPC communication error between server and worker" mapped to HTTP 400. This variant is the correct target for the `From<IpcError>` conversion — the task explicitly instructs to reuse it, not invent a new variant.

The existing error tests in `crates/anvilml-core/tests/error_tests.rs` establish the crate's test style: `#[tokio::test]` async functions with doc comments, constructing error variants and asserting their behaviour. The `anvilml-ipc` crate already has `crates/anvilml-ipc/tests/roundtrip_tests.rs` using the same pattern.

The `thiserror` crate is already used by `anvilml-core` at version `2.0.18`, confirming the derive macro API (`#[derive(Error)]`, `#[error("...")]`) is established in this workspace.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source     | Feature flags confirmed |
|--------|-----------|-----------------|----------------|------------------------|
| crate  | thiserror | 2.0.18          | rust-docs MCP  | none                   |

`thiserror` 2.0.18 was confirmed via `rust-docs_get_crate_version` — released 2026-01-18, MSRV 1.68. This matches the version already pinned in `anvilml-core/Cargo.toml`. The derive macro API (`#[derive(thiserror::Error)]`, `#[error("...")]`, `#[from]`) is unchanged from v1 to v2.

## Approach

1. **Add `thiserror` dependency to `crates/anvilml-ipc/Cargo.toml`.** Add `thiserror = "2.0.18"` under the existing `[dependencies]` section, matching the version already used by `anvilml-core`. This is a compile-time-only dependency (the derive macro expands at compile time); no runtime features are needed.

2. **Create `crates/anvilml-ipc/src/error.rs`.** Implement the `IpcError` enum with the following structure:
   - Derive `Debug`, `Clone`, `thiserror::Error`, and `Display`.
   - Define six variants with `#[error("...")]` attributes for human-readable messages:
     - `BindFailed(String)` — "bind failed: {0}"
     - `SendFailed(String)` — "send failed: {0}"
     - `RecvFailed(String)` — "recv failed: {0}"
     - `SerializationFailed(String)` — "serialization failed: {0}"
     - `PayloadTooLarge { actual: usize, max: usize }` — "payload too large: {actual} > {max}"
     - `UnknownWorker(String)` — "unknown worker: {0}"
   - The `Display` derive is provided by `thiserror::Error` via the `#[error("...")]` attributes — no separate `impl Display` is needed.
   - Add a crate-level `//!` doc comment describing the module's purpose per `ANVILML_DESIGN.md §8.4`'s layout.
   - Add a `///` doc comment on the enum and each variant explaining what it represents.

3. **Implement `From<IpcError> for AnvilError`.** Import `anvilml_core::AnvilError` and implement the conversion. Every `IpcError` variant maps to `AnvilError::Ipc(String)` by formatting the error's own `Display` output into the string. This ensures callers outside `anvilml-ipc` receive a single domain-level error type without needing to know about IPC-specific error details. The rationale for mapping all variants to `Ipc(String)` rather than creating separate `AnvilError` variants: the task explicitly states to reuse the existing variant, and the `AnvilError::Ipc` doc comment already covers "Internal IPC communication error" generically.

4. **Update `crates/anvilml-ipc/src/lib.rs`.** Add `mod error;` (private module declaration) and `pub use error::IpcError;` (public re-export). Preserve the existing `pub mod ws;` and `pub use ws::broadcaster::EventBroadcaster;` lines. The file should remain well under the 80-line hard cap.

5. **Create `crates/anvilml-ipc/tests/error_tests.rs`.** Write >=5 tests:
   - One test per `IpcError` variant verifying its `Display` output matches the expected format (5 string-variant tests).
   - One test for `PayloadTooLarge` variant verifying its `Display` output includes both `actual` and `max` values.
   - One test for `From<IpcError> for AnvilError` verifying that converting each variant produces `AnvilError::Ipc(_)` with the correct message.
   - Use `#[tokio::test]` for async tests if needed (though Display and From are synchronous, so `#[test]` suffices).
   - Follow the doc-comment style from `anvilml-core/tests/error_tests.rs`.

## Public API Surface

| Item | Crate/Module Path | Description |
|------|-------------------|-------------|
| `pub enum IpcError` | `anvilml_ipc::IpcError` | IPC-specific error enum with 6 variants, derives `Debug`, `Clone`, `Error`, `Display` |
| `impl From<IpcError> for AnvilError` | `anvilml_ipc` (via `IpcError`) | Converts any `IpcError` variant to `AnvilError::Ipc(String)` |
| `pub use error::IpcError` | `anvilml_ipc::IpcError` (re-export) | Makes `IpcError` available as `anvilml_ipc::IpcError` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-ipc/src/error.rs` | `IpcError` enum with `thiserror::Error` derive and `From<IpcError> for AnvilError` |
| MODIFY | `crates/anvilml-ipc/src/lib.rs` | Add `mod error;` and `pub use error::IpcError;` |
| MODIFY | `crates/anvilml-ipc/Cargo.toml` | Add `thiserror = "2.0.18"` dependency |
| CREATE | `crates/anvilml-ipc/tests/error_tests.rs` | >=5 tests for Display output and From conversion |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `crates/anvilml-ipc/tests/error_tests.rs` | `test_bind_failed_display` | `IpcError::BindFailed("addr".to_string()).to_string()` returns `"bind failed: addr"` | `cargo test -p anvilml-ipc --test error_tests test_bind_failed_display` exits 0 |
| `crates/anvilml-ipc/tests/error_tests.rs` | `test_send_failed_display` | `IpcError::SendFailed("timeout".to_string()).to_string()` returns `"send failed: timeout"` | `cargo test -p anvilml-ipc --test error_tests test_send_failed_display` exits 0 |
| `crates/anvilml-ipc/tests/error_tests.rs` | `test_recv_failed_display` | `IpcError::RecvFailed("closed".to_string()).to_string()` returns `"recv failed: closed"` | `cargo test -p anvilml-ipc --test error_tests test_recv_failed_display` exits 0 |
| `crates/anvilml-ipc/tests/error_tests.rs` | `test_serialization_failed_display` | `IpcError::SerializationFailed("bad msgpack".to_string()).to_string()` returns `"serialization failed: bad msgpack"` | `cargo test -p anvilml-ipc --test error_tests test_serialization_failed_display` exits 0 |
| `crates/anvilml-ipc/tests/error_tests.rs` | `test_payload_too_large_display` | `IpcError::PayloadTooLarge { actual: 1024, max: 512 }` formats both values in output | `cargo test -p anvilml-ipc --test error_tests test_payload_too_large_display` exits 0 |
| `crates/anvilml-ipc/tests/error_tests.rs` | `test_unknown_worker_display` | `IpcError::UnknownWorker("gpu:3".to_string()).to_string()` returns `"unknown worker: gpu:3"` | `cargo test -p anvilml-ipc --test error_tests test_unknown_worker_display` exits 0 |
| `crates/anvilml-ipc/tests/error_tests.rs` | `test_from_ipc_error_to_anvil_error` | Converting every `IpcError` variant via `From` produces `AnvilError::Ipc(_)` with correct message | `cargo test -p anvilml-ipc --test error_tests test_from_ipc_error_to_anvil_error` exits 0 |

Acceptance command: `cargo test -p anvilml-ipc --test error_tests` exits 0 (all 7 tests pass).

## CI Impact

No CI changes required. The new test file lives in the crate's `tests/` directory, which `cargo test --workspace --features mock-hardware` (the CI job's test command) already picks up automatically for all crate members. No new file types, gates, or CI configurations are introduced.

## Platform Considerations

None identified. The `IpcError` enum and its `Display`/`From` implementations are pure data transformations with no platform-specific code paths, no `#[cfg(unix)]` or `#[cfg(windows)]` guards required. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `thiserror` 2.0 API may have changed from v1 — the `#[error("...")]` struct-field interpolation syntax (e.g. `{actual} > {max}`) might not work with named struct fields. | Low | Medium | Verify via MCP: confirmed `thiserror` 2.0.18 supports struct-field interpolation in `#[error("...")]` attributes. If it does not, use `Display` impl with `write!` instead. |
| Adding `thiserror` to `anvilml-ipc` may introduce a transitive dependency conflict with `anvilml-core`'s existing `thiserror` pin, since both are path dependencies in the same workspace. | Low | Medium | Using the exact same version string (`"2.0.18"`) as `anvilml-core` avoids this — cargo will deduplicate the dependency. The workspace already uses `anvilml-core` as a dependency of `anvilml-ipc`. |
| The `From<IpcError> for AnvilError` conversion may conflict with an existing blanket impl if one exists. | Very Low | High | `AnvilError` is in `anvilml-core`, `IpcError` is in `anvilml-ipc` — they are different crates, so there can be no coherence conflict (both types are local to their respective crates). |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-ipc --test error_tests` exits 0
- [ ] `grep -c "^fn test_" crates/anvilml-ipc/tests/error_tests.rs` returns >= 5 (counts test function definitions)
- [ ] `grep -c "IpcError::" crates/anvilml-ipc/tests/error_tests.rs` returns >= 6 (each variant is exercised)
- [ ] `grep "pub use error::IpcError" crates/anvilml-ipc/src/lib.rs` matches (re-export is present)
- [ ] `grep "thiserror" crates/anvilml-ipc/Cargo.toml` matches (dependency is declared)
- [ ] `wc -l crates/anvilml-ipc/src/lib.rs` returns <= 80 (lib.rs line-count cap)
