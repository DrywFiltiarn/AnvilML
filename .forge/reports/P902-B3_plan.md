# Plan Report: P902-B3

| Field       | Value                                         |
|-------------|-----------------------------------------------|
| Task ID     | P902-B3                                       |
| Phase       | 902 — Stabilisation Retrofit                  |
| Description | anvilml-server: add TraceLayer request/response DEBUG logging middleware |
| Depends on  | none                                          |
| Project     | anvilml                                       |
| Planned at  | 2026-06-08T18:30:00Z                          |
| Attempt     | 1                                             |

## Objective

Add `tower-http`'s `TraceLayer` as the first middleware in `anvilml-server`'s router stack, fulfilling `ANVILML_DESIGN.md §10.2`. This provides structured DEBUG-level logging of every incoming HTTP request (method, URI) and every outgoing response (status code, latency) via the `tracing` crate — covering all 10 routes with zero handler changes.

## Scope

### In Scope
- Add `tower-http = { version = "0.6", features = ["trace"] }` to workspace `[workspace.dependencies]` in root `Cargo.toml`.
- Add `tower-http = { workspace = true }` to `crates/anvilml-server/Cargo.toml` `[dependencies]`.
- Add `use tower_http::trace::TraceLayer;` import in `crates/anvilml-server/src/lib.rs`.
- Append `.layer(TraceLayer::new_for_http())` to the router chain in `build_router()`, immediately after `.with_state(state_arc)`.
- Bump `anvilml-server` crate version from `0.1.3` to `0.1.4` (patch bump, per ENVIRONMENT.md §10).

### Out of Scope
- No changes to any file in `handlers/`.
- No new handler files or route definitions.
- No changes to WebSocket middleware stack (other layers like `SetRequestIdLayer`, `CompressionLayer`, `CorsLayer` remain unimplemented per design; TraceLayer is the first of four planned layers).
- No new test files. Existing tests verify functional correctness unchanged.
- No changes to CI configuration, OpenAPI spec, or documentation files.

## Approach

1. **Workspace dependency declaration.** In root `Cargo.toml`, add a single line under `[workspace.dependencies]`:
   ```toml
   tower-http = { version = "0.6", features = ["trace"] }
   ```
   This makes the crate available to all workspace members via `{ workspace = true }`. The `"trace"` feature is the only one needed — it provides `TraceLayer` and its configuration types.

2. **Server crate dependency.** In `crates/anvilml-server/Cargo.toml`, add a single line under `[dependencies]`:
   ```toml
   tower-http = { workspace = true }
   ```
   This follows the project's established convention of declaring all dependencies through the workspace manifest.

3. **Router layer insertion.** In `crates/anvilml-server/src/lib.rs`:
   - Add `use tower_http::trace::TraceLayer;` to the existing imports (after `use axum::{...}`).
   - In `build_router()`, change the router chain from:
     ```rust
     Router::new()
         .route(...)
         ...
         .with_state(state_arc)
     ```
     to:
     ```rust
     Router::new()
         .route(...)
         ...
         .with_state(state_arc)
         .layer(TraceLayer::new_for_http())
     ```
   - The `TraceLayer` is placed outermost (last in the chain), so it wraps all request processing. This matches the design spec's "outermost first" ordering.

4. **Version bump.** Increment `anvilml-server` version from `"0.1.3"` to `"0.1.4"` in `crates/anvilml-server/Cargo.toml`. Source files are modified, so a patch bump is required per ENVIRONMENT.md §10.

5. **Verify compilation and tests.** Run `cargo test -p anvilml-server --features mock-hardware` to confirm all existing tests pass with the new layer in place. TraceLayer adds only logging instrumentation; it does not modify request/response bodies or status codes, so functional behavior is unchanged.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `Cargo.toml` | Add `tower-http = { version = "0.6", features = ["trace"] }` to `[workspace.dependencies]` |
| Modify | `crates/anvilml-server/Cargo.toml` | Add `tower-http = { workspace = true }` to `[dependencies]`; bump version `0.1.3 → 0.1.4` |
| Modify | `crates/anvilml-server/src/lib.rs` | Add `TraceLayer` import and `.layer(TraceLayer::new_for_http())` after `.with_state(state_arc)` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-server/src/lib.rs` (inline tests) | `health_returns_200` | Router returns 200 with correct JSON body through the new middleware layer |
| `crates/anvilml-server/src/lib.rs` (inline tests) | `env_returns_200_with_stub_report` | `/v1/system/env` handler works correctly with TraceLayer active |
| `crates/anvilml-server/src/lib.rs` (inline tests) | `system_returns_200_with_hardware_info` | Hardware info endpoint responds correctly under mock-hardware feature |
| `crates/anvilml-server/src/lib.rs` (inline tests) | `get_model_returns_404_when_missing` | 404 error path works through TraceLayer (error responses still logged) |
| `crates/anvilml-server/src/lib.rs` (inline tests) | `rescan_returns_202` | POST handler returns 202 Accepted through TraceLayer |
| `crates/anvilml-server/src/lib.rs` (inline tests) | `workers_endpoint_returns_200` | Workers endpoint status code and response shape correct through TraceLayer |

No new test files are written. The existing six inline tests in `lib.rs` cover the router construction path, and `TraceLayer::new_for_http()` is a no-op for functionality — it only emits tracing events.

## CI Impact

No CI configuration changes required. The task adds a dependency and a middleware layer; the existing CI gates (`cargo clippy`, `cargo test`, format check) cover the modified files. No new jobs or steps are needed. The OpenAPI drift gate is not triggered because no handler signatures or utoipa annotations change.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `TraceLayer::new_for_http()` API shape differs from expectations (e.g. requires configuration) | Low | Medium | The task spec explicitly uses `TraceLayer::new_for_http()` — this is the default builder method that produces sensible defaults. If compilation fails, inspect tower-http 0.6 docs and adjust with minimal config. |
| Tower version conflict: `tower-http 0.6` requires a different `tower` major version than existing `tower = "0.5"` | Low | High | Per task constraints note, both `axum 0.8` and `tower-http 0.6` depend on `tower 0.5`, so no conflict. Workspace already declares `tower = { version = "0.5" }`. |
| Existing tests fail due to TraceLayer altering response timing or behavior | Very Low | Medium | `TraceLayer::new_for_http()` is transparent — it does not modify request/response bodies, headers (beyond tracing), or status codes. Tests should pass unchanged. If a test fails, diagnose and fix minimally. |
| Version bump conflict with concurrent tasks on anvilml-server | Low | Low | The patch version (`0.1.4`) is deterministic for this session. Forge orchestrator handles merge conflicts if needed. |

## Acceptance Criteria

- [ ] `tower-http` dependency added to workspace `[workspace.dependencies]` in root `Cargo.toml` with `version = "0.6"` and `features = ["trace"]`
- [ ] `tower-http = { workspace = true }` present in `crates/anvilml-server/Cargo.toml` `[dependencies]`
- [ ] `use tower_http::trace::TraceLayer;` import added to `lib.rs`
- [ ] `.layer(TraceLayer::new_for_http())` appended after `.with_state(state_arc)` in `build_router()`
- [ ] `anvilml-server` crate version bumped from `0.1.3` to `0.1.4`
- [ ] No changes to any file in `handlers/` or any other source directory outside the three listed files
- [ ] `cargo test -p anvilml-server --features mock-hardware` exits 0
- [ ] `cargo clippy -p anvilml-server --features mock-hardware -- -D warnings` exits 0
