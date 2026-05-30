# Tasks: Phase 008 — Launcher & Graceful Shutdown

| Field            | Value                                                                        |
|------------------|------------------------------------------------------------------------------|
| Phase            | 008                                                                          |
| Name             | Launcher & Graceful Shutdown                                                 |
| ANVIL Milestone  | M4 (part 2)                                                                  |
| Status           | Draft                                                                        |
| Depends on phases| 1, 2, 3, 4, 5, 6, 7                                                          |
| Task file        | `forge/tasks/tasks_phase008.json`                                            |
| Design reference | `ANVILML_DESIGN.md` §16 (Launcher), §6.1 (Preflight), §20 (Testing), §22.4 |

---

## Overview

Phase 008 turns the collection of library crates into a functioning binary. The `backend/src/main.rs` launcher implements the twelve-step startup sequence, the cross-platform graceful shutdown signal handler, and the full integration test suite in `backend/tests/`. This phase completes M4.

The M4 exit criterion is: "All `api_*.rs` integration tests green; release binary starts, browser opens, graceful shutdown." The release binary is delivered by P8-A1 and P8-A2; the signal handling by P8-A3; the integration tests by P8-B1. When all four tasks pass, the entire Rust backend is verified end-to-end under `--features mock-hardware`.

The shutdown signal handling is the most platform-sensitive Rust code in the project. The design requires joining three signal sources: `ctrl_c` (both platforms), `SIGTERM` (Unix only), and `ctrl_close`/`ctrl_shutdown` (Windows only). These require conditional compilation because the Windows signal APIs do not exist on Unix and vice versa. The implementation must use `#[cfg(unix)]` / `#[cfg(windows)]` correctly — not a runtime OS check — to avoid compile errors on either platform.

The integration tests use an in-process axum test server (not a real bound port) for all HTTP tests. The WebSocket test (`api_ws.rs`) does require a real bound port for `tokio-tungstenite`. Use `axum_test` or bind to `127.0.0.1:0` (OS-assigned port) and retrieve the actual address for the test client.

---

## Group Reference

| Group | Subsystem        | Tasks          | Summary                                               |
|-------|------------------|----------------|-------------------------------------------------------|
| A     | backend/main.rs  | P8-A1 … P8-A3  | CLI, config, startup sequence, graceful shutdown      |
| B     | backend/tests/   | P8-B1          | Full integration test suite + openapi-diff CI gate    |

---

## Prerequisites

- P7-B2 complete: `anvilml-server` is fully implemented including the router, all handlers, WebSocket, and frontend serving.
- `cargo run -p anvilml-openapi` already generates `backend/openapi.json`.

---

## Contract Documents Applicable to This Phase

| Document section          | Relevant tasks | What must match                                                          |
|---------------------------|----------------|--------------------------------------------------------------------------|
| `ANVILML_DESIGN.md` §16.1 | P8-A1          | CLI flags: `--config`, `--host`, `--port`, `--no-browser`, `--log-format` |
| `ANVILML_DESIGN.md` §16.2 | P8-A2          | Startup sequence steps 1–11 in order                                    |
| `ANVILML_DESIGN.md` §16.3 | P8-A3          | Shutdown sequence steps 1–6; cross-platform signal sources               |
| `ANVILML_DESIGN.md` §6.1  | P8-A2          | Preflight checks and `EnvReport` fields                                  |
| `ANVILML_DESIGN.md` §20   | P8-B1          | Integration test list; parity test; WS timing assertions                 |
| `ANVILML_DESIGN.md` §22.4 | P8-A3          | Windows: `ctrl_close` + `ctrl_shutdown`; Unix: `SIGTERM`                 |

---

## Task Descriptions

### Group A — Launcher

#### P8-A1: backend/main.rs — CLI parsing and config loading

**Goal:** Implement the binary's entry point with CLI argument parsing and layered configuration loading.

**Files to create or modify:**
- `backend/src/main.rs` — replace stub with real implementation
- `backend/Cargo.toml` — add `clap` (derive feature), a config/layering crate (e.g. `config` or `figment`), `tracing-subscriber` (env-filter + json features), `anvilml-server`

**Key implementation notes:**
- CLI with `clap` derive: `--config <PATH>` (default `./anvilml.toml`), `--host <IP>`, `--port <u16>`, `--no-browser` (flag), `--log-format plain|json` (default `plain`).
- Config resolution (lowest to highest priority): built-in defaults → parse `anvilml.toml` from `--config` path (warn but do not fail if absent) → `ANVILML_*` environment variables → `--host`/`--port` CLI overrides. The result is a fully populated `ServerConfig`.
- Tracing subscriber: init with `EnvFilter::from_env("ANVILML_LOG")` falling back to `RUST_LOG`, default `info`. Select formatter: `--log-format plain` uses `tracing_subscriber::fmt()` (human-readable); `--log-format json` uses `.json()`.
- `fn main()` should call `tokio::main` and delegate to an async `run(args)` function so errors can be propagated cleanly.

**Acceptance criterion:** `cargo build --release` exits 0. `./target/release/sindristudio --help` prints usage.

---

#### P8-A2: backend/main.rs — startup sequence

**Goal:** Implement the full 11-step startup sequence that wires all subsystems together into a running server.

**Files to create or modify:**
- `backend/src/main.rs` — `async fn run(args: Args) -> Result<(), anyhow::Error>`

**Key implementation notes:**
- Step 3: `anvilml_registry::db::open(&cfg.db_path)` → `SqlitePool`.
- Step 4: `UPDATE jobs SET status='Failed', error='server_restart' WHERE status IN ('Running','Queued')`. Run before any worker spawns.
- Step 5: `anvilml_hardware::detect_all_devices(&cfg)` → `HardwareInfo`.
- Step 6 (Python preflight):
  1. Resolve interpreter path (§6, cross-platform).
  2. If path does not exist: `env_report.preflight_ok = false, reason = "python_missing"`.
  3. Else run `{interpreter} --version`; log version at `info`. If output does not contain `Python 3.12`, log `warn` but do not abort.
  4. If `ANVILML_WORKER_MOCK` is not set: run `{interpreter} -c "import torch; print(torch.__version__)"`. On non-zero exit: `env_report.preflight_ok = false, reason = "torch_unavailable"`.
  5. All preflight failures are soft: the server still starts; `POST /v1/jobs` returns 503.
- Step 7: `registry.rescan(&cfg.model_dirs)` spawned as a non-blocking `tokio::spawn` task.
- Step 8: `WorkerPool::spawn_all(&hw, &cfg)`.
- Step 9: construct `AppState`, call `scheduler.start_dispatch_loop()`, call `start_system_stats_tick()`.
- Step 10: `axum::serve(TcpListener::bind(addr)?, router)` as a `tokio::spawn` task, not blocking.
- Step 11: if `!args.no_browser && cfg.frontend.mode != Headless`: call `open::that(format!("http://{}:{}", host, port))`. Add `open` crate dep.

**Acceptance criterion:** `cargo run -- --no-browser` starts in under 3 s (mock-hardware env), responds `200` to `GET /health`, and exits cleanly on Ctrl-C.

---

#### P8-A3: backend/main.rs — graceful shutdown (cross-platform signal handling)

**Goal:** Implement the shutdown future that handles all platform-specific signal sources and runs the six-step clean shutdown sequence.

**Files to create or modify:**
- `backend/src/main.rs` — `async fn shutdown_signal()` and shutdown sequence in `run()`

**Key implementation notes:**
- Build a combined shutdown future:
  ```rust
  async fn shutdown_signal() {
      let ctrl_c = tokio::signal::ctrl_c();
      #[cfg(unix)]
      let sigterm = async {
          tokio::signal::unix::signal(SignalKind::terminate())
              .expect("failed to install SIGTERM handler")
              .recv().await;
      };
      #[cfg(not(unix))]
      let sigterm = std::future::pending::<()>();
      #[cfg(windows)]
      let ctrl_close = async {
          tokio::signal::windows::ctrl_close()
              .expect("failed to install ctrl_close handler")
              .recv().await;
      };
      #[cfg(not(windows))]
      let ctrl_close = std::future::pending::<()>();
      tokio::select! {
          _ = ctrl_c => {}
          _ = sigterm => {}
          _ = ctrl_close => {}
      }
  }
  ```
- On signal: (1) log `"Shutting down..."`; (2) stop accepting new job submissions by setting a shutdown flag that `POST /v1/jobs` checks → return `503`; (3) `workers.shutdown_all()` (sends Shutdown, waits 10 s, force-kills); (4) close `SqlitePool` (WAL flush); (5) exit 0.
- On Windows, `ctrl_shutdown` (sent when the system is shutting down) should also be handled. Add a fourth branch using `tokio::signal::windows::ctrl_shutdown()`.

**Acceptance criterion:** `cargo test --workspace --features mock-hardware -- shutdown` exits 0. On Linux: `kill -SIGTERM <pid>` triggers clean shutdown in integration test.

---

### Group B — Integration Tests

#### P8-B1: Integration tests — api_*.rs suite and CI openapi-diff gate

**Goal:** Implement the complete Rust integration test suite that exercises every REST endpoint and the WebSocket stream with an in-process mock server, and activate the CI openapi-diff gate.

**Files to create or modify:**
- `backend/tests/api_health.rs`
- `backend/tests/api_jobs.rs`
- `backend/tests/api_models.rs`
- `backend/tests/api_workers.rs`
- `backend/tests/api_artifacts.rs`
- `backend/tests/api_ws.rs`
- `.github/workflows/ci.yml` — update `openapi-diff` job to run `cargo run -p anvilml-openapi` before `git diff --exit-code backend/openapi.json`

**Key implementation notes:**
- All HTTP integration tests: build the axum router from `anvilml_server::build_router(app_state)` using a real in-process `AppState` backed by an in-memory SQLite (`sqlite::memory:`) and `ANVILML_WORKER_MOCK=1` workers.
- `api_health.rs`: assert `GET /health` returns `200` with `status=ok` and a numeric `uptime_s`.
- `api_jobs.rs`: (1) submit with a valid graph → 202; (2) submit with an unknown node type → 422 `invalid_graph`; (3) list returns the submitted job; (4) cancel a queued job → 202; (5) cancel the same job again → 409 `job_not_cancellable`; (6) delete a completed job → 204; (7) delete a running job → 409 `job_active`.
- `api_models.rs`: list returns empty; POST `/v1/models/rescan` → 202; list after rescan returns results from tempdir.
- `api_workers.rs`: list returns workers with status `Initializing` or `Idle`; restart → 202.
- `api_artifacts.rs`: list returns empty; save an artifact via `ArtifactStore::save` directly; list returns it; GET by hash returns PNG with correct `Content-Type` and `Cache-Control`.
- `api_ws.rs`: use `tokio-tungstenite` with a real bound TCP port. Connect to `/v1/events`; assert a `system.stats` JSON frame arrives within 6 s. Submit a mock job; assert `job.queued`, `job.started`, `job.image_ready`, `job.completed` frames arrive in order within 15 s.
- `ANVILML_WORKER_MOCK=1` must be set for the test process or all worker tests will fail due to missing torch.

**Acceptance criterion:** `cargo test --workspace --features mock-hardware` exits 0 with all `api_*` tests passing on both Linux and Windows CI runners.

---

## Phase Acceptance Criteria

```
cargo build --release
cargo test --workspace --features mock-hardware
cargo run -p anvilml-openapi && git diff --exit-code backend/openapi.json
cargo clippy --workspace --features mock-hardware -- -D warnings
```

---

## Known Constraints and Gotchas

- `tokio::signal::windows::ctrl_close` and `ctrl_shutdown` are Windows-only APIs. Wrap them in `#[cfg(windows)]` blocks. Using `std::future::pending::<()>()` as the Unix fallback for the `ctrl_close` branch ensures the `tokio::select!` macro compiles on both platforms without special-casing the select arms.
- The `sqlite::memory:` URL (`sqlite::memory:`) creates a per-connection in-memory database. `sqlx::SqlitePool` with `max_connections(1)` is required for in-memory SQLite so all connections share the same database state — using multiple connections will create multiple independent in-memory databases.
- The `api_ws.rs` test must bind to `127.0.0.1:0` and read the actual port from the `TcpListener` after bind, before passing it to `tokio-tungstenite`. Do not hardcode a port number; port collisions cause flaky test failures in CI.
- Ghost-job reset (startup step 4) must run before any test that queries job status, because in-memory SQLite is fresh for each test — this is fine, but the migration must still run to create the schema before the reset UPDATE executes. Ensure `db::open` is called before the reset UPDATE in the test setup helper.
- The `open` crate's browser launch must be guarded by `--no-browser` in tests, or it will attempt to open a browser window in the headless CI environment and may hang or produce confusing output.
