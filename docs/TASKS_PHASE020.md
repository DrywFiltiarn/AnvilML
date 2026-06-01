# Tasks: Phase 020 — OpenAPI & Launcher Polish

| Field | Value |
|-------|-------|
| Phase | 020 |
| Name | OpenAPI & Launcher Polish |
| Milestone group | Production surface |
| Depends on phases | 1-19 |
| Task file | `forge/tasks/tasks_phase020.json` |
| Tasks | 4 |

## Overview

Phase 20 finalises the developer-facing surface: `utoipa` annotations on every handler, the `anvilml-openapi` generator that emits the committed `backend/openapi.json`, browser auto-open at startup, and the CI openapi-diff gate plus the Python pytest job. After this phase the API is fully documented by a machine-readable spec that CI keeps honest.

Every task in this phase implements **one module or one endpoint** plus its test. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the **Runnable Proof** below passes.

## Tasks

| Task | Module / File | Summary |
|------|-------------|---------|
| P20-A1 | `crates/anvilml-server/src/handlers/*.rs` | anvilml-server: utoipa annotations on all handlers + schemas |
| P20-A2 | `crates/anvilml-openapi/src/main.rs` | anvilml-openapi: generate backend/openapi.json |
| P20-A3 | `backend/src/main.rs` | anvilml: browser auto-open at startup (unless --no-browser/Headless) |
| P20-A4 | `.github/workflows/ci.yml` | anvilml: CI openapi-diff gate + python-worker pytest job |

## Task details

#### P20-A1: anvilml-server: utoipa annotations on all handlers + schemas

- **Prereqs:** P19-A3
- **Tags:** —

Add #[utoipa::path(...)] annotations to every REST handler (health, system, env, jobs CRUD+cancel, models, workers, artifacts) with correct method/path/responses. Ensure all anvilml-core request/response types derive utoipa::ToSchema (already added in phase 3 - verify). No behavior change. cargo build -p anvilml-server --features mock-hardware exits 0; cargo clippy clean.

#### P20-A2: anvilml-openapi: generate backend/openapi.json

- **Prereqs:** P20-A1
- **Tags:** reasoning

Implement anvilml-openapi/src/main.rs: #[derive(OpenApi)] struct referencing all handler paths + component schemas (incl WsEvent variants as schemas). Serialize to pretty JSON, write backend/openapi.json. Verify: cargo run -p anvilml-openapi writes a non-empty backend/openapi.json containing all /v1 paths and the error response shape; commit the file.

#### P20-A3: anvilml: browser auto-open at startup (unless --no-browser/Headless)

- **Prereqs:** P20-A2
- **Tags:** —

Add `open` crate to backend. In main.rs after the server is bound and /health is confirmed reachable: unless args.no_browser or frontend.mode==Headless, call open::that(format!('http://{}:{}', host, port)); log if it fails (do not abort). Note: default mode is Headless so no browser opens by default; only opens when a custom frontend is configured (Local/Remote). Verify: with frontend.mode=local, cargo run opens the browser; default (headless) and --no-browser do not.

#### P20-A4: anvilml: CI openapi-diff gate + python-worker pytest job

- **Prereqs:** P20-A3
- **Tags:** —

Update .github/workflows/ci.yml: add to rust-linux job a step `cargo run -p anvilml-openapi` then `git diff --exit-code backend/openapi.json` (fails if stale). Add job python-worker (ubuntu): pip install -r worker/requirements/base.txt then ANVILML_WORKER_MOCK=1 pytest worker/tests/. Verify: locally cargo run -p anvilml-openapi && git diff --exit-code backend/openapi.json passes; pytest worker/tests passes.


## Runnable Proof

Generate the OpenAPI spec and confirm the diff gate passes; confirm browser auto-open.

```bash
cargo run -p anvilml-openapi
git diff --exit-code backend/openapi.json    # exits 0 (committed spec matches)
jq '.paths | keys' backend/openapi.json       # lists all /v1 paths + /health
cargo run --features mock-hardware            # default Local mode opens the browser
cargo run --features mock-hardware -- --no-browser   # does NOT open a browser
```

Expected: `openapi.json` regenerates identically (diff gate green) and lists every endpoint; starting the server opens the default browser to the served UI unless `--no-browser` or Headless. Phase done when the openapi-diff gate passes in CI and auto-open behaves correctly.
