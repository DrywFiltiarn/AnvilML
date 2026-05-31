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

Phase 19 implements the three `frontend.mode` options: `Local` (ServeDir + SPA fallback, with a friendly warning page when the directory is missing), `Headless` (API only), and `Remote` (reverse-proxy to a dev server). After this phase the running binary can serve a frontend, and API routes always take priority over the catch-all.

Every task in this phase implements **one module or one endpoint** plus its test. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the **Runnable Proof** below passes.

## Tasks

| Task | Module / File | Summary |
|------|---------------|---------|
| P19-A1 | `src/frontend.rs` | anvilml-server: frontend Local mode (ServeDir + SPA fallback) |
| P19-A2 | anvilml-server | anvilml-server: frontend Headless mode (no catch-all) |
| P19-A3 | anvilml-server | anvilml-server: frontend Remote mode (reverse proxy) |

## Task details

#### P19-A1: anvilml-server: frontend Local mode (ServeDir + SPA fallback)

- **Prereqs:** P18-A4
- **Tags:** reasoning

Add tower-http ServeDir feature. Create src/frontend.rs: add_frontend_route(router,mode)->Router. Local{path}: ServeDir at / with SPA fallback to {path}/index.html; if path missing log warn + serve inline HTML page '<h1>AnvilML</h1><p>Frontend not found. API at /v1/.</p>'. Register as lowest-priority catch-all AFTER all /v1 and /health routes. Wire in main.rs from cfg.frontend.mode. Verify: create ./bloomery/index.html with 'hello'; cargo run; browser http://127.0.0.1:8488/ shows it; /health and /v1/system still work.

#### P19-A2: anvilml-server: frontend Headless mode (no catch-all)

- **Prereqs:** P19-A1
- **Tags:** —

In frontend.rs handle FrontendMode::Headless: register no catch-all; non-API paths return 404. Verify: set frontend.mode=headless in anvilml.toml (or ANVILML_FRONTEND__MODE=headless); cargo run; browser / returns 404; /health still 200.

#### P19-A3: anvilml-server: frontend Remote mode (reverse proxy)

- **Prereqs:** P19-A2
- **Tags:** reasoning

Add hyper client. In frontend.rs handle FrontendMode::Remote{url}: catch-all handler proxies non-API requests to {url}{path}, forwarding headers (strip hop-by-hop), rewriting Host, streaming response back. Dev-use tolerance is fine. Verify: run any static server on :5173 (e.g. python -m http.server 5173); set frontend.mode=remote url=http://localhost:5173; cargo run; browser http://127.0.0.1:8488/ shows the proxied page; /v1/system still served locally.


## Runnable Proof

Serve a local page and confirm it loads while the API still works.

```bash
mkdir -p bloomery && echo '<h1>AnvilML UI</h1>' > bloomery/index.html
# anvilml.toml: [frontend] mode = "local" path = "./bloomery"
cargo run --features mock-hardware
```

Open `http://127.0.0.1:8488/` in a browser. Expected: the `AnvilML UI` page renders; `curl /health` and `curl /v1/system` still return JSON (API beats the catch-all). Switch to `mode = "headless"` -> `/` returns 404 but `/health` still works. Switch to `mode = "remote"` with a dev server on :5173 -> `/` shows the proxied page. Phase done when all three modes behave as described.
