# Plan Report: P19-A3

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P19-A3                                            |
| Phase       | 019 — Frontend Serving                            |
| Description | anvilml-server: frontend Remote mode (reverse proxy) |
| Depends on  | P19-A2                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-06-11T22:05:00Z                              |
| Attempt     | 1                                                 |

## Objective

Implement the `FrontendMode::Remote { url }` arm in `frontend.rs` so that when the
frontend mode is set to `remote`, all non-API HTTP requests are reverse-proxied to the
configured URL (e.g. a Vite dev server on `http://localhost:5173`). The proxy must use
`hyper`/`hyper-util` as the HTTP client, strip hop-by-hop headers, rewrite the `Host`
header to the target, and stream the upstream response body back to the client. API
routes (`/v1/*`, `/health`) are always served locally by AnvilML; only unmatched paths
are proxied.

## Scope

### In Scope
- Add `hyper` and `hyper-util` workspace dependency references to `anvilml-server/Cargo.toml`.
- Extend `add_frontend_route()` in `crates/anvilml-server/src/frontend.rs` to handle
  `FrontendMode::Remote { url }` by mounting a catch-all proxy handler via
  `router.nest_service("/", service_fn(proxy_handler))`.
- The proxy handler:
  - Extracts the request path and query from the original `Uri`.
  - Constructs a new upstream URI: `{url}{path}{?query}`.
  - Sends the request via `hyper::Client` with `hyper-util` legacy adapter.
  - Strips hop-by-hop headers (`connection`, `keep-alive`, `transfer-encoding`, `te`,
    `trailers`, `upgrade`) from the upstream response before forwarding.
  - Rewrites the `Host` header to the upstream host.
  - Streams the upstream response body back to the client.
  - Logs proxy errors at WARN level.
- Unit test `test_frontend_remote`: bind a `tokio::net::TcpListener` on a random port,
  serve a static HTML response, configure `FrontendMode::Remote { url }` pointing to it,
  and verify `GET /` returns 200 with the proxied body.
- Bump `anvilml-server` crate patch version (0.1.16 → 0.1.17).

### Out of Scope
- BloomeryUI integration or configuration (SindriStudio's responsibility).
- TLS/mTLS for the reverse proxy (dev-use only).
- Request body buffering or size limits beyond what hyper provides by default.
- WebSocket proxy support (future task).
- Any changes to `lib.rs` wiring (already wired; `add_frontend_route` called before
  `.with_state()` — no modification needed).
- Changes to `anvilml-core` or any other crate.

## Approach

1. **Verify workspace deps exist.** The root `Cargo.toml` already declares
   `hyper = { version = "1", features = ["client", "http1"] }` and
   `hyper-util = { version = "0.1", features = ["client", "client-legacy", "http1"] }`
   (lines 51–52). Confirm no new feature flags are needed — the existing ones suffice for
   a basic HTTP/1 reverse proxy client.

2. **Add deps to `anvilml-server/Cargo.toml`.** Add two lines under `[dependencies]`:
   ```toml
   hyper = { workspace = true }
   hyper-util = { workspace = true }
   ```
   Verify no version conflict with the workspace declarations.

3. **Implement the Remote arm in `frontend.rs`.**
   - Add imports: `hyper`, `hyper_util`, `http::{Request, Uri}`, `tower::service_fn`,
     `anvilml_core::FrontendMode`.
   - In the `FrontendMode::Remote { url }` arm of `add_frontend_route()`:
     - Log at DEBUG: `mounting remote frontend proxy to {url}`.
     - Create a `hyper::Client` with `hyper_util::client::legacy::Connector::default()`.
     - Define an inner `proxy_handler` async closure (capturing `url` and `client`):
       1. Extract the original request's `uri` and `method`.
       2. Parse the path and query from the request URI.
       3. Construct the upstream URI: `url.join(path)?` with query preserved.
       4. Build a new `Request<Body>`:
          - Method from original.
          - URI set to upstream.
          - Copy all original headers except hop-by-hop ones.
          - Set `Host` header to the upstream host (from `url`).
       5. Send the request via `client.request(upstream_req)`.
       6. On success: build a response from the upstream response, stripping hop-by-hop
          headers and preserving status + body. Stream body with `hyper_util::body::SyncBoxBody`
          or `axum::body::Body::from_stream`.
       7. On error: return 502 Bad Gateway with the error description.
     - Mount via `router.nest_service("/", svc)`.
   - Ensure the match arm for `Remote` is exhaustive (replacing the current `..` catchall).

4. **Write the unit test `test_frontend_remote`.**
   - Use `tokio::net::TcpListener::bind("127.0.0.1:0")` to get a random available port.
   - Spawn an async task that accepts one connection, reads the request line, and replies
     with a minimal HTTP/1.1 200 response containing `AnvilML Remote Proxy Test` in the body.
   - Build the router with `FrontendMode::Remote { url }` pointing to
     `http://127.0.0.1:{port}`.
   - Send `GET /` via `app.oneshot(...)`.
   - Assert status 200 and body contains `AnvilML Remote Proxy Test`.
   - Assert that `/health` still returns 200 (API routes take priority over nest_service).

5. **Bump crate version.** Change `anvilml-server/Cargo.toml` version from `0.1.16` to
   `0.1.17`.

6. **Verify.** Run `cargo test -p anvilml-server --lib -- frontend` and confirm all tests
   pass. Run `cargo clippy -p anvilml-server --features mock-hardware -- -D warnings`
   and confirm zero warnings.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/Cargo.toml` | Add `hyper` and `hyper-util` workspace deps; bump version 0.1.16 → 0.1.17 |
| Modify | `crates/anvilml-server/src/frontend.rs` | Implement `Remote` arm with proxy handler; add `test_frontend_remote` unit test |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|-----------------|
| `crates/anvilml-server/src/frontend.rs` | `test_frontend_remote` | A proxied `GET /` returns 200 with the upstream body; API routes like `/health` are not proxied |
| `crates/anvilml-server/src/frontend.rs` | `test_frontend_local_serves_fixture` | (existing) Local mode still works |
| `crates/anvilml-server/src/frontend.rs` | `test_frontend_local_missing_path` | (existing) Missing path fallback still works |
| `crates/anvilml-server/src/frontend.rs` | `test_frontend_headless` | (existing) Headless mode still returns 404 for `/` |

## CI Impact

No CI changes required. The `hyper` and `hyper-util` workspace deps are already declared
in the root `Cargo.toml` with stable versions (1.x and 0.1.x). No new CI gates or
workflow files are modified. The existing CI gates (format, clippy, tests, platform
cross-check) will cover the new code.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `hyper::Client` lifetime / connector setup fails to compile with axum types | Medium | High | Use `hyper_util::client::legacy::Client` which is designed for axum integration; follow the pattern from `tower-http` examples. |
| Body type mismatch between hyper response and axum `Body` | Medium | Medium | Use `hyper_util::body::SyncBoxBody` or convert via `axum::body::Body::from_stream` with `futures_util::stream::once`. |
| URI join fails for malformed remote URLs | Low | Medium | The `url.join()` method returns a `Result`; propagate as 400 Bad Request in the proxy handler. |
| Hop-by-hop header stripping misses a header | Low | Low | Reference RFC 7230 §6.1 list; include all six headers in a `HashSet` for filtering. |
| Test TCP listener race condition (port not ready before request) | Medium | Medium | Use `tokio::time::sleep` or a oneshot channel to signal when the listener is ready before building the app and sending the request. |

## Acceptance Criteria

- [ ] `hyper` and `hyper-util` added as workspace deps in `anvilml-server/Cargo.toml`
- [ ] `add_frontend_route()` handles `FrontendMode::Remote { url }` with `nest_service`
- [ ] Proxy strips hop-by-hop headers: `connection`, `keep-alive`, `transfer-encoding`, `te`, `trailers`, `upgrade`
- [ ] Proxy rewrites `Host` header to upstream target
- [ ] Proxy streams response body back to client
- [ ] Unit test `test_frontend_remote` passes (200 with proxied body)
- [ ] Existing tests (`test_frontend_local_serves_fixture`, `test_frontend_local_missing_path`, `test_frontend_headless`) still pass
- [ ] `cargo clippy -p anvilml-server --features mock-hardware -- -D warnings` exits 0
- [ ] `anvilml-server` crate version bumped to `0.1.17`
