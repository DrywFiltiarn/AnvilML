# Plan Report: P19-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P19-A1                                            |
| Phase       | 019 — Frontend Serving                            |
| Description | anvilml-server: frontend Local mode (ServeDir + SPA fallback) |
| Depends on  | P18-A4, P904-A3                                   |
| Project     | anvilml                                           |
| Planned at  | 2026-06-11T20:25:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create `crates/anvilml-server/src/frontend.rs` implementing `pub fn add_frontend_route<S>(router: Router<S>, mode: &FrontendMode) -> Router<S>` that handles `FrontendMode::Local { path }` by mounting `ServeDir` with SPA fallback via `nest_service`, and returns the router unchanged for all other modes. Wire the function into `lib.rs`'s `build_router` before `.with_state()`.

## Scope

### In Scope
- Create `crates/anvilml-server/src/frontend.rs` with `add_frontend_route` function
- Local mode: if path exists, mount `ServeDir::new(&path).fallback(ServeFile::new(path.join("index.html")))` via `router.nest_service("/", svc)`
- Local mode: if path missing, log `warn!` and mount inline-HTML fallback via `service_fn` + `nest_service`
- Other modes (`Headless`, `Remote`): return `router` unchanged (handled in later tasks)
- Wire `mod frontend;` and `add_frontend_route` call in `lib.rs`
- Unit tests in `frontend.rs` under `#[cfg(test)] mod tests`
- Bump `anvilml-server` patch version from `0.1.14` to `0.1.15`

### Out of Scope
- Headless mode implementation (P19-A2)
- Remote mode / reverse proxy implementation (P19-A3)
- Any changes to `anvilml-core`, `backend/`, or other crates
- Integration/e2e tests (covered by phase Runnable Proof in later tasks)
- Modifying `anvilml.toml` or `docs/ENVIRONMENT.md` (no config surface change)

## Approach

1. **Create `crates/anvilml-server/src/frontend.rs`** with imports for `axum::Router`, `tower_http::serve_dir::ServeDir`, `tower_http::serve_file::ServeFile`, `tower::service_fn`, `tracing`, and `anvilml_core::FrontendMode`.

2. **Implement `add_frontend_route`** with the `FrontendMode` match:
   - `Local { path }` branch:
     a. Check `path.is_dir()` with `std::fs::metadata`
     b. If exists: build `ServeDir::new(&path).fallback(ServeFile::new(path.join("index.html")))`, then `router.nest_service("/", svc)`
     c. If missing: log `tracing::warn!(path = %path.display(), "frontend path {:?} not found, serving inline fallback", path)`, build `service_fn` returning `200` HTML body `<h1>AnvilML</h1><p>Frontend not found. API at /v1/.</p>`, then `router.nest_service("/", svc)`
   - `Headless` | `Remote`: `router` (return unchanged)

3. **Wire into `lib.rs`**:
   a. Add `mod frontend;` at top of file
   b. In `build_router`, clone `state.config` before `Arc::new(state)` — `let config = state.config.clone();`
   c. After all route definitions, before `.with_state(state_arc)`: `let router = frontend::add_frontend_route(router, &config.frontend.mode);`

4. **Add unit tests** in `#[cfg(test)] mod tests`:
   a. `test_frontend_local_serves_fixture`: resolve fixture path via `CARGO_MANIFEST_DIR` (parent×2 → repo root → `test-frontend`), call `add_frontend_route` with `FrontendMode::Local { path }`, build in-process app, `GET /` → assert 200 and body contains `"AnvilML Test Frontend"`
   b. `test_frontend_local_missing_path`: pass non-existent `PathBuf`, call `add_frontend_route`, build in-process app, `GET /` → assert 200 and body contains `"Frontend not found"`

5. **Bump `anvilml-server` version** in `crates/anvilml-server/Cargo.toml` from `0.1.14` to `0.1.15`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-server/src/frontend.rs` | New module with `add_frontend_route` and unit tests |
| Modify | `crates/anvilml-server/src/lib.rs` | Add `mod frontend;`, clone config, wire `add_frontend_route` |
| Modify | `crates/anvilml-server/Cargo.toml` | Bump patch version `0.1.14 → 0.1.15` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| `crates/anvilml-server/src/frontend.rs` | `test_frontend_local_serves_fixture` | Local mode serves fixture HTML, returns 200, body contains expected text |
| `crates/anvilml-server/src/frontend.rs` | `test_frontend_local_missing_path` | Missing path falls back to inline HTML, returns 200, body contains "Frontend not found" |

## CI Impact

No CI workflow file changes. The new tests run under the existing `cargo test --workspace --features mock-hardware` gate. The `tower` and `tower-http` dependencies are already declared in the workspace and anvilml-server manifests (tower-http with `cors` and `fs` features). No new dependencies required.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `tower_http::serve_dir::ServeDir` or `ServeFile` API shape differs from expectations in tower-http 0.6 | Low | Medium | Verify import paths and method names during implementation; tower-http 0.6 `fs` feature provides both |
| `nest_service` signature mismatch with tower 0.5 | Low | Medium | The workspace declares tower 0.5; `nest_service` takes a service implementing `Service<Request>` — `service_fn` satisfies this |
| Test fixture path resolution fails at compile time | Low | Medium | Use the exact `CARGO_MANIFEST_DIR` parent traversal pattern documented in TASKS_PHASE019.md; the fixture file is committed at repo root |
| `build_router` change breaks existing tests that call `build_router` without frontend config | Low | Low | Frontend mode defaults to `Headless` in `ServerConfig::default()` which returns router unchanged — existing tests unaffected |

## Acceptance Criteria

- [ ] `crates/anvilml-server/src/frontend.rs` exists with `pub fn add_frontend_route<S>(router: Router<S>, mode: &FrontendMode) -> Router<S>` where `S: Clone + Send + Sync + 'static`
- [ ] Local mode with existing path serves files and SPA fallback via `ServeDir` + `ServeFile` using `nest_service` (not `route_service`)
- [ ] Local mode with missing path logs `warn!` and serves inline HTML via `service_fn` + `nest_service`
- [ ] `Headless` and `Remote` modes return router unchanged
- [ ] `lib.rs` includes `mod frontend;` and wires `add_frontend_route` after routes, before `.with_state()`
- [ ] Both unit tests pass: `cargo test -p anvilml-server --lib -- frontend`
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` passes clean
- [ ] `anvilml-server` Cargo.toml version bumped to `0.1.15`
