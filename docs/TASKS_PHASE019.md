# Tasks: Phase 019 — Frontend Serving

| Field | Value |
|-------|-------|
| Phase | 019 |
| Name | Frontend Serving |
| Milestone group | Production surface |
| Depends on phases | 1-18 |
| Task file | `forge/tasks/tasks_phase019.json` |
| Tasks | 3 |

## Overview

Phase 19 implements the three `frontend.mode` options: `Local` (ServeDir + SPA fallback,
with a friendly warning page when the directory is missing), `Headless` (API only), and
`Remote` (reverse-proxy to a dev server). After this phase the running binary can serve a
frontend, and API routes always take priority over the catch-all.

Every task in this phase implements **one module or one endpoint** plus its test. No task
touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates;
the phase as a whole is only complete when the **Runnable Proof** below passes.

## Tasks

| Task | Module / File | Summary |
|------|-------------|---------|
| P19-A1 | `crates/anvilml-server/src/frontend.rs` | anvilml-server: frontend Local mode (ServeDir + SPA fallback) |
| P19-A2 | `crates/anvilml-server/src/frontend.rs` | anvilml-server: frontend Headless mode (no catch-all) |
| P19-A3 | `crates/anvilml-server/src/frontend.rs` | anvilml-server: frontend Remote mode (reverse proxy) |

## Known Constraints and Gotchas

### `nest_service` not `route_service` — CRITICAL

Use `router.nest_service("/", svc)` for the frontend catch-all. **Never use
`route_service("/", svc)`.**

`route_service` does an exact-path match — `route_service("/", svc)` matches only the
literal `/` and does not strip the prefix before passing the request to the inner service.
`ServeDir` receives the full unstripped path, issues a 301 redirect to add a trailing
slash, and in-process test clients do not follow redirects — producing a 301 or 404 in
tests and a broken experience in production for every path other than `/`.

`nest_service` strips the mount prefix from the URI before forwarding to the inner
service and acts as a true catch-all. Routes registered **before** the `nest_service`
call always take priority automatically — no extra ordering logic is needed.

### `ServeDir` has no `.index_file()` method

Do not call `.index_file()` on `ServeDir` — this method does not exist in tower-http 0.6.
`append_index_html_on_directories` defaults to `true` and must **not** be called
explicitly. The SPA fallback for unmatched sub-paths (e.g. `/about`) is a separate
concern handled by `.fallback(ServeFile::new(path.join("index.html")))`, which preserves
the 200 status. Do **not** use `.not_found_service()` for this — it forces a 404 status
on the fallback response, which breaks SPA routing.

### Unit test fixture — `test-frontend/index.html`

The committed file `test-frontend/index.html` at the repository root is the permanent
test fixture for Local mode tests. Unit tests must resolve its path at compile time:

```rust
let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
    .parent().unwrap()  // crates/anvilml-server -> crates
    .parent().unwrap()  // crates -> repo root
    .join("test-frontend");
```

Do **not** use `tempfile` to create a temporary directory and write `index.html` into it.
The file already exists on disk; tests must use it directly.

### `add_frontend_route` function signature

The helper must be generic over the router state so it can be called before
`.with_state()`:

```rust
pub fn add_frontend_route<S>(router: Router<S>, mode: &FrontendMode) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
```

Wire it in `build_router` **before** `.with_state(state_arc)`:

```rust
let config = state.config.clone();   // clone before move into Arc
let state_arc = Arc::new(state);
let router = Router::new()
    .route("/health", ...)
    .route("/v1/...", ...);
let router = frontend::add_frontend_route(router, &config.frontend.mode);
router
    .with_state(state_arc)
    .layer(CorsLayer::permissive())
    .layer(TraceLayer::new_for_http())
```

### Inline HTML fallback (path missing)

When the configured path does not exist use `tower::service_fn`:

```rust
use tower::service_fn;

let svc = service_fn(|_req: axum::extract::Request| async {
    let body = "<h1>AnvilML</h1><p>Frontend not found. API at /v1/.</p>";
    Ok::<_, std::convert::Infallible>(
        axum::response::Response::builder()
            .status(200)
            .header("content-type", "text/html; charset=utf-8")
            .body(axum::body::Body::from(body))
            .unwrap(),
    )
});
router.nest_service("/", svc)
```

Do not implement a custom `tower::Service` struct — `service_fn` is simpler and avoids
lifetime complexity.

---

## Task details

#### P19-A1: anvilml-server: frontend Local mode (ServeDir + SPA fallback)

- **Prereqs:** P18-A4, P904-A3
- **Tags:** reasoning

Create `crates/anvilml-server/src/frontend.rs`. Implement:

```rust
pub fn add_frontend_route<S>(router: Router<S>, mode: &FrontendMode) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
```

For `FrontendMode::Local { path }`:

- If `path` exists as a directory: mount
  `ServeDir::new(&path).fallback(ServeFile::new(path.join("index.html")))`
  via `router.nest_service("/", svc)`.
- If `path` does not exist: log
  `warn!("frontend path {:?} not found, serving inline fallback", path)`
  and mount a `service_fn` inline-HTML response via `nest_service`.

For all other modes (`Headless`, `Remote`): return `router` unchanged (handled in later
tasks).

Wire in `lib.rs`: add `mod frontend;`, clone `state.config` before `Arc::new(state)`,
call `frontend::add_frontend_route(router, &config.frontend.mode)` after all `/v1` and
`/health` routes, before `.with_state(state_arc)`.

The `fs` feature for `tower-http` is already present in `anvilml-server/Cargo.toml`
(`features = ["cors", "fs"]`). Verify only — do not add a duplicate entry.

**Unit tests** (in `frontend.rs` under `#[cfg(test)] mod tests`):

- `test_frontend_local_serves_fixture`: resolve the fixture path via
  `CARGO_MANIFEST_DIR` (see Known Constraints above), call `add_frontend_route` with
  `FrontendMode::Local { path: fixture }`, build an in-process app, `GET /` → assert
  200 and body contains the expected text from `test-frontend/index.html`.
- `test_frontend_local_missing_path`: pass a non-existent path → `GET /` → assert 200
  and body contains `"Frontend not found"`.

Verify: `cargo test -p anvilml-server --lib -- frontend` all pass; `cargo clippy` clean.

#### P19-A2: anvilml-server: frontend Headless mode (no catch-all)

- **Prereqs:** P19-A1
- **Tags:** —

In `frontend.rs` add the `FrontendMode::Headless` arm to `add_frontend_route`: return
`router` unchanged. No catch-all is registered; non-API paths fall through to axum's
default 404.

**Unit test**: `test_frontend_headless` — pass `FrontendMode::Headless`, build in-process
app, `GET /` → assert 404; `GET /health` wired separately → assert 200.

Verify: `cargo test -p anvilml-server --lib -- frontend` all pass; `cargo clippy` clean.

#### P19-A3: anvilml-server: frontend Remote mode (reverse proxy)

- **Prereqs:** P19-A2
- **Tags:** reasoning

Add `hyper` and `hyper-util` to `anvilml-server/Cargo.toml` if not already present;
confirm workspace-level declarations exist.

In `frontend.rs` add the `FrontendMode::Remote { url }` arm: mount a catch-all proxy
handler via `router.nest_service("/", service_fn(proxy_handler))` — **not**
`route_service`. The proxy handler forwards the request to `{url}{path}`, strips
hop-by-hop headers (`connection`, `keep-alive`, `transfer-encoding`, `te`, `trailers`,
`upgrade`), rewrites the `Host` header to the target host, and streams the response body
back. Dev-use tolerance is acceptable; this proxies a custom frontend dev server only —
not BloomeryUI (SindriStudio's responsibility).

**Unit test**: `test_frontend_remote` — bind a `tokio::net::TcpListener` on a random
port serving a static HTML response, pass its address as
`FrontendMode::Remote { url: "http://127.0.0.1:{port}".parse().unwrap() }`, build
in-process app, `GET /` → assert 200 and proxied body received.

Verify: `cargo test -p anvilml-server --lib -- frontend` all pass; `cargo clippy` clean.

---

## Runnable Proof

`test-frontend/index.html` must already exist in the repository (committed fixture).

```bash
# Confirm fixture is present
test -f test-frontend/index.html && echo "fixture present"

# --- 1. Local mode ---
ANVILML_FRONTEND__MODE=local \
ANVILML_FRONTEND__PATH=./test-frontend \
cargo run --features mock-hardware &
SERVER=$!
sleep 2

curl -s http://127.0.0.1:8488/         | grep -i "anvilml"   # fixture page renders
curl -s http://127.0.0.1:8488/health   | grep '"status"'     # API still works
curl -s http://127.0.0.1:8488/v1/system | grep '"host"'      # API still works
kill $SERVER

# --- 2. Headless mode ---
ANVILML_FRONTEND__MODE=headless \
cargo run --features mock-hardware &
SERVER=$!
sleep 2

curl -o /dev/null -s -w "%{http_code}" http://127.0.0.1:8488/ | grep 404
curl -s http://127.0.0.1:8488/health | grep '"status"'
kill $SERVER

# --- 3. Remote mode ---
python -m http.server 5173 --directory test-frontend &
PYSERVER=$!

ANVILML_FRONTEND__MODE=remote \
ANVILML_FRONTEND__URL=http://localhost:5173 \
cargo run --features mock-hardware &
SERVER=$!
sleep 2

curl -s http://127.0.0.1:8488/          | grep -i "anvilml"  # proxied page renders
curl -s http://127.0.0.1:8488/v1/system | grep '"host"'      # local API not proxied
kill $SERVER $PYSERVER
```

Phase done when all three modes behave as described: Local serves `test-frontend/`,
Headless returns 404 for `/` while `/health` stays 200, Remote proxies to the dev server
while `/v1/*` is served locally.