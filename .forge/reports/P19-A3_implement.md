# Implementation Report: P19-A3

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P19-A3                                      |
| Phase         | 019 — Frontend Serving                      |
| Description   | anvilml-server: frontend Remote mode (reverse proxy) |
| Implemented   | 2026-06-12T01:15:00Z                        |
| Status        | COMPLETE                                    |

## Summary

Implemented the `FrontendMode::Remote { url }` arm in `add_frontend_route()` to mount a
reverse-proxy catch-all handler using `hyper`/`hyper-util` as the HTTP client. The proxy
strips hop-by-hop headers, rewrites the `Host` header to the upstream target, and forwards
the upstream response back to the client. A unit test `test_frontend_remote` verifies
end-to-end proxying with a mock TCP server and confirms API routes (e.g. `/health`) are
not intercepted by the proxy.

## Resolved Dependencies

| Type   | Name         | Version resolved | Source         |
|--------|-------------|------------------|----------------|
| crate  | hyper        | 1.10.1           | lockfile       |
| crate  | hyper-util   | 0.1.20           | lockfile       |
| crate  | http         | 1.4.1            | lockfile       |
| crate  | http-body-util | 0.1            | lockfile       |
| crate  | url          | 2.5.8            | workspace dep  |

Note: `hyper` and `hyper-util` were already declared in the workspace root
`Cargo.toml` (lines 51–52). The `http` crate was moved from workspace dev-deps to
workspace deps (runtime use). `http-body-util` was also promoted to workspace deps.
`url` was already in workspace deps. No new versions were resolved via MCP — all
versions confirmed from the lockfile as per rule 6.4 (MCP unavailable for Rust).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | `Cargo.toml` | Moved `http` from dev-deps to workspace deps section |
| Modify | `crates/anvilml-server/Cargo.toml` | Added `http`, `http-body-util`, `hyper`, `hyper-util`, `url` workspace deps; bumped version 0.1.16 → 0.1.17 |
| Modify | `crates/anvilml-server/src/frontend.rs` | Implemented `Remote` arm with `proxy_handler`; added `test_frontend_remote` unit test; added `HOP_BY_HOP` constant and `BodyExt` import |

## Commit Log

```
 .forge/state/CURRENT_TASK.md          |   6 +-
 .forge/state/state.json               |  13 +-
 Cargo.lock                            |   6 +-
 Cargo.toml                            |   2 +-
 crates/anvilml-server/Cargo.toml      |   7 +-
 crates/anvilml-server/src/frontend.rs | 254 +++++++++++++++++++++++++++++++++-
 6 files changed, 272 insertions(+), 16 deletions(-)
```

## Test Results

```
running 4 tests
test frontend::tests::test_frontend_local_missing_path ... ok
test frontend::tests::test_frontend_headless ... ok
test frontend::tests::test_frontend_remote ... ok
test frontend::tests::test_frontend_local_serves_fixture ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 37 filtered out
```

Full workspace test suite: 279 tests passed, 0 failed.

## Format Gate

```
(no output — exit 0)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.51s

# 2. Mock-hardware Windows cross-check (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 10.97s

# 3. Real-hardware Linux check
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.19s

# 4. Real-hardware Windows cross-check (x86_64-pc-windows-gnu)
Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.26s
```

## Project Gates

```
# Gate 1 — Config Surface Sync
running 1 test
test test_toml_key_set_matches_default ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 4 filtered out
```

## Deviations from Plan

- **Used `fallback_service` instead of `nest_service`**: axum 0.8 does not support
  `nest_service` at the root path (`"/"`). The error was "Nesting at the root is no
  longer supported. Use fallback_service instead." The `fallback_service` approach is
  functionally equivalent — API routes (`/health`, `/v1/*`) take priority over the
  fallback, and unmatched paths are served by the proxy.
- **Body conversion via `collect().to_bytes()` instead of streaming**: `hyper::body::Incoming`
  does not implement `Stream`, and `axum::body::Body::from_stream` requires `Send + Sync`
  bounds that `Incoming` cannot satisfy. Used `http_body_util::BodyExt::collect()` to
  buffer the body, then `Body::from(bytes)` to construct the axum body. This is a
  pragmatic deviation — the proxy still works correctly but buffers the full response
  body in memory rather than streaming.
- **Moved `http` and `http-body-util` to workspace runtime deps**: These were previously
  only in workspace dev-deps. Since `http::Uri` is used at runtime for the proxy handler,
  they needed to be promoted to runtime workspace deps.

## Blockers

None.
