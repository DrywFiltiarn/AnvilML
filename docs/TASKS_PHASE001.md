# Tasks: Phase 001 — Walking Skeleton

| Field | Value |
|-------|-------|
| Phase | 001 |
| Name | Walking Skeleton |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 0 |

## Overview

Phase 001 produces the first runnable artifact: a binary called `anvilml` that binds an axum HTTP server on `127.0.0.1:8488` and responds to `GET /health` with a JSON body containing status, version, and uptime. This is the walking skeleton — the thinnest possible slice that proves the runtime, server framework, and build toolchain are all working together correctly.

Nothing beyond `/health` exists after this phase. There is no config loading, no workers, no database. The sole purpose is to prove that the build produces a binary that can accept HTTP connections and respond correctly.

The Runnable Proof — a `curl` to `/health` — is the benchmark against which every subsequent phase is measured. If something regresses and `/health` stops working, a developer can immediately identify which phase introduced the regression.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | anvilml-server | P1-A1 … P1-A3 | AppState, health handler, build_router |
| B | backend binary | P1-B1 | main.rs: bind, serve, tokio runtime |

## Prerequisites

Phase 000 must be complete: workspace compiles, all 9 crate skeletons exist, CI workflow in place.

## Interfaces and Contracts

| Contract document | Relevant tasks | What must match |
|-------------------|---------------|-----------------|
| `ANVILML_DESIGN.md §12.4` | P1-A2 | `GET /health` response shape: `{status, version, uptime_s}` |

## Task Descriptions

### Group A — anvilml-server

#### P1-A1: anvilml-server: AppState struct

**Goal:** Define `AppState` in `crates/anvilml-server/src/state.rs` holding `start_time: Instant` and `version: String`. This is the shared state all handlers will receive.

**Files to create:**
- `crates/anvilml-server/src/state.rs` — `pub struct AppState { start_time: std::time::Instant, version: String }` with `pub fn new(version: impl Into<String>) -> Self`. Derive or implement `Clone`.

**Acceptance criterion:** `cargo test -p anvilml-server` exits 0.

#### P1-A2: anvilml-server: GET /health handler

**Goal:** Implement the `/health` handler in `crates/anvilml-server/src/handlers/health.rs` returning `{"status":"ok","version":"<ver>","uptime_s":<N>}`.

**Files to create:**
- `crates/anvilml-server/src/handlers/mod.rs` — `pub mod health;`
- `crates/anvilml-server/src/handlers/health.rs` — async handler extracting `State<AppState>`, computing `uptime_s`, returning `axum::Json`. Unit test using `axum::body::to_bytes` asserts 200 and correct shape.

**Acceptance criterion:** `cargo test -p anvilml-server -- health` exits 0 with at least 1 test passing.

#### P1-A3: anvilml-server: build_router

**Goal:** Wire `AppState` and the health handler into an axum `Router` via `pub fn build_router(state: AppState) -> Router` in `crates/anvilml-server/src/lib.rs`.

**Files to modify:**
- `crates/anvilml-server/src/lib.rs` — declare modules, implement `build_router`, add integration test.

**Acceptance criterion:** `cargo test -p anvilml-server` exits 0 with all tests passing.

### Group B — backend binary

#### P1-B1: backend: main.rs bind and serve

**Goal:** Implement `backend/src/main.rs` to create an `AppState`, call `build_router`, bind `TcpListener` on `127.0.0.1:8488`, and serve with `axum::serve`. Log the bind address at INFO.

**Files to modify:**
- `backend/src/main.rs` — tokio `#[tokio::main]` entry point; bind; serve; INFO log.

**Acceptance criterion:** `cargo run --features mock-hardware &` + `curl -s http://127.0.0.1:8488/health | python3 -m json.tool` prints valid JSON with `status` key; process exits 0 on Ctrl-C.

## Phase Acceptance Criteria

```bash
cargo build --features mock-hardware
cargo run --features mock-hardware &
sleep 2
curl -s http://127.0.0.1:8488/health | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['status']=='ok'"
kill %1
```

## Known Constraints and Gotchas

- `AppState` must implement `Clone` because axum's `State` extractor requires it.
- Use `std::time::Instant` for uptime, not `tokio::time::Instant` — the latter is not `Send + Sync` in all configurations.
- The version string should be read from `env!("CARGO_PKG_VERSION")` so it tracks the workspace version automatically.
