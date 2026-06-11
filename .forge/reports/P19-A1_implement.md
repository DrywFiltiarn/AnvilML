# Implementation Report: P19-A1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P19-A1                                            |
| Phase       | 019 — Frontend Serving                            |
| Description | anvilml-server: frontend Local mode (ServeDir + SPA fallback) |
| Implemented | 2026-06-11T23:15:00Z                              |
| Status      | COMPLETE                                          |

## Summary

Implemented `add_frontend_route` in a new `frontend.rs` module for `anvilml-server`. The function handles `FrontendMode::Local` by mounting `ServeDir` with SPA fallback via `fallback_service` (axum 0.8 API), and returns the router unchanged for `Headless` and `Remote` modes. Wired the function into `lib.rs`'s `build_router`. Added two unit tests that both pass. Bumped `anvilml-server` version from `0.1.14` to `0.1.15`.

## Resolved Dependencies

| Type   | Name         | Version resolved | Source         |
|--------|--------------|------------------|----------------|
| crate  | tower-http   | 0.6.11           | lockfile       |
| crate  | axum         | 0.8              | lockfile       |

No new dependencies were added. The `tower-http` `fs` feature was already declared in the workspace and `anvilml-server` manifests. The `tower` crate (0.5) was already declared.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Create | `crates/anvilml-server/src/frontend.rs` | New module with `add_frontend_route` function and unit tests |
| Modify | `crates/anvilml-server/src/lib.rs` | Added `mod frontend;`, cloned config in `build_router`, wired `add_frontend_route` call |
| Modify | `crates/anvilml-server/Cargo.toml` | Bumped patch version `0.1.14 → 0.1.15` |

## Commit Log

```
 .forge/reports/P19-A1_plan.md         |  94 +++++++++++++++++++++
 .forge/state/CURRENT_TASK.md          |   6 +-
 .forge/state/state.json               |  13 +--
 Cargo.lock                            |   2 +-
 crates/anvilml-server/Cargo.toml      |   2 +-
 crates/anvilml-server/src/frontend.rs | 148 ++++++++++++++++++++++++++++++++++
 crates/anvilml-server/src/lib.rs      |   8 +-
 7 files changed, 260 insertions(+), 13 deletions(-)
```

## Test Results

```
     Running unittests src/lib.rs (target/debug/deps/anvilml_server-d17e628a4c1f2305)

running 40 tests
test frontend::tests::test_frontend_local_missing_path ... ok
test frontend::tests::test_frontend_local_serves_fixture ... ok
test artifact::store::tests::delete_for_job_empty_returns_zero ... ok
...
test result: ok. 40 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Full workspace: 271 tests passed, 0 failed across all crates (anvilml-core, anvilml-hardware, anvilml-ipc, anvilml-registry, anvilml-scheduler, anvilml-server, anvilml-worker, backend, anvilml-openapi).

## Format Gate

```
(Not applicable — pass 2 re-run after drift: `cargo fmt --all` applied formatting fixes, then `cargo fmt --all -- --check` exited 0 with no output.)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
cargo check --workspace --features mock-hardware
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.26s

# 2. Mock-hardware Windows cross-check
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.94s

# 3. Real-hardware Linux check
cargo check --bin anvilml
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.60s

# 4. Real-hardware Windows cross-check
cargo check --bin anvilml --target x86_64-pc-windows-gnu
  Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.33s
```

All four platform cross-checks exited 0.

## Project Gates

```
cargo test -p backend --features mock-hardware -- test_toml_key_set_matches_default
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

Config surface sync gate passed.

## Deviations from Plan

- **`nest_service` → `fallback_service`**: The plan specified using `router.nest_service("/", svc)` for both the `ServeDir` case and the inline-HTML fallback case. However, axum 0.8 (the workspace version) no longer supports `nest_service` at the root path — it panics with "Nesting at the root is no longer supported. Use fallback_service instead." The implementation uses `router.fallback_service(svc)` instead, which provides the same catch-all-at-root behavior. This is the correct axum 0.8 API for this use case.
- **`tower_http::serve_dir` → `tower_http::services`**: The plan specified imports from `tower_http::serve_dir::ServeDir` and `tower_http::serve_file::ServeFile`. In tower-http 0.6 (the workspace version), these types are at `tower_http::services::ServeDir` and `tower_http::services::ServeFile`. The implementation uses the correct module paths.
- **`lib.rs` router variable assignment**: The original `build_router` used a fluent chain ending with `.with_state(state_arc)`. Since `add_frontend_route` needs to intercept the router before `.with_state()`, the chain was split into a `let mut router = Router::new()...` assignment followed by `router = frontend::add_frontend_route(router, ...)` and then `.with_state(state_arc)`.

## Blockers

None.
