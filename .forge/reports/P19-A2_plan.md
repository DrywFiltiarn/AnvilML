# Plan Report: P19-A2

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P19-A2                                            |
| Phase       | 019 — Frontend Serving                            |
| Description | anvilml-server: frontend Headless mode (no catch-all) |
| Depends on  | P19-A1                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-06-11T21:20:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add a dedicated `FrontendMode::Headless` arm to `add_frontend_route()` in
`crates/anvilml-server/src/frontend.rs` so that when the frontend is configured as
headless, the router is returned unchanged — no catch-all route is registered, and
non-API paths fall through to axum's default 404. Add a unit test verifying this
behaviour.

## Scope

### In Scope
- Modify the match arm in `add_frontend_route()` to separate `Headless` from `Remote`
- Add unit test `test_frontend_headless` in `frontend.rs` under `#[cfg(test)]`
  - Builds an in-process app with `FrontendMode::Headless`
  - `GET /` → assert 404
  - `GET /health` → assert 200
- No source code changes beyond the match arm separation (the existing `router` passthrough
  already implements the correct behaviour for headless)

### Out of Scope
- Any change to `lib.rs` (routing wiring already correct — P19-A1 handles this)
- Any change to `anvilml-core` config or `FrontendMode` enum
- Remote mode implementation (P19-A3)
- Integration/acceptance tests (those are covered by the Runnable Proof in TASKS_PHASE019.md)
- No crate version bump (no source files modified, only a test added)

## Approach

1. **Separate the match arm** in `add_frontend_route()` (`frontend.rs`, line 46):
   - Before: `FrontendMode::Headless | FrontendMode::Remote { .. } => router,`
   - After: `FrontendMode::Headless => router,` followed by a new arm for `Remote { .. }`
     that also returns `router` unchanged.
   - This makes the headless path explicit and self-documenting.

2. **Add unit test** `test_frontend_headless` in `frontend.rs` under `#[cfg(test)] mod tests`:
   - Import `std::path::PathBuf` (already imported) and reuse `repo_root()` and `build_app()`
     helpers from the existing test module.
   - Create a minimal router with only the `/health` route:
     ```rust
     let router = Router::new().route("/health", axum::routing::get(|| async { "ok" }));
     ```
   - Call `add_frontend_route(router, &FrontendMode::Headless)`.
   - Build in-process app via `build_app()`.
   - Assert `GET /` returns 404.
   - Assert `GET /health` returns 200 with body `"ok"`.

3. **Verify**: Run `cargo test -p anvilml-server --lib -- frontend` to confirm all
   frontend tests pass (both existing Local tests and the new Headless test).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/frontend.rs` | Separate `Headless` arm from `Remote` in `add_frontend_route()`; add `test_frontend_headless` unit test |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-server/src/frontend.rs` | `test_frontend_headless` | Headless mode returns 404 for `/` and 200 for `/health`; no catch-all is mounted |

## CI Impact

No CI changes required. The task modifies only existing source and test files within
`anvilml-server`. The existing CI gates (format, clippy, tests, cross-checks, config
drift) will cover the change automatically when the full test suite runs.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Existing Local-mode tests break due to match arm reordering | Low | Medium | The change is purely additive (splitting one arm into two equivalent arms); the `router` passthrough value is identical. Run all `frontend` tests to confirm. |
| Axum version mismatch — `GET /` returns 200 instead of 404 | Low | High | Verify with the actual axum version in use; if axum has changed default behaviour, adjust the test assertion accordingly. |

## Acceptance Criteria

- [ ] `FrontendMode::Headless` has its own dedicated arm in `add_frontend_route()` (not grouped with `Remote`)
- [ ] Unit test `test_frontend_headless` exists and passes: `GET /` → 404, `GET /health` → 200
- [ ] `cargo test -p anvilml-server --lib -- frontend` — all tests pass (existing Local tests + new Headless test)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` — zero warnings
