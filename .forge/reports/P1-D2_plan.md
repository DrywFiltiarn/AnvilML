# Plan Report: P1-D2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P1-D2                                       |
| Phase       | 001 — Repository Scaffold                   |
| Description | Runnable Proof: live binary answers /health over real TCP |
| Depends on  | P1-D1, P1-C1                                |
| Project     | anvilml                                     |
| Planned at  | 2026-06-26T15:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Produce Phase 1's Runnable Proof transcript by building the `anvilml` binary in release mode, launching it in the background, and confirming via a real `curl` request over TCP that `GET /health` returns HTTP 200. This task introduces no source changes — it verifies that the endpoint wired by P1-D1 is live and reachable from the compiled binary.

## Scope

### In Scope
- Build `anvilml` binary in release mode (`cargo build --release -p anvilml`).
- Launch the binary in the background on the default address (`127.0.0.1:8488`).
- Send `curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8488/health` and record the `200` response.
- Terminate the background process (`kill %1`).
- Record the literal terminal output in the implementation report.

### Out of Scope
None. This task's `defers_to` field is empty (`[]`), and no functionality is deferred.

## Existing Codebase Assessment

**What already exists:** The full stack is in place from prior tasks. `backend/src/cli.rs` defines `Cli` with `host` (default `"127.0.0.1"`) and `port` (default `8488`) via clap derive. `backend/src/main.rs` parses CLI args, builds the router via `anvilml_server::build_router()`, binds a `TcpListener` on `cli.host:cli.port`, and serves with `axum::serve()` raced against a shutdown signal. `crates/anvilml-server/src/handlers/health.rs` implements `async fn health() -> StatusCode` returning `StatusCode::OK`. `crates/anvilml-server/src/lib.rs`'s `build_router()` registers `GET /health`. A unit test in `crates/anvilml-server/tests/health_tests.rs` verifies the route in-process.

**Established patterns:** The codebase uses clap derive for CLI parsing, axum for HTTP routing, tokio for async runtime, and tracing for structured logging. Tests in `crates/*/tests/` use the crate's public API directly. The `anvilml.toml` config file exists but is NOT yet loaded by main.rs — config loading is Phase 2 scope. At this phase, main.rs relies solely on CLI defaults, which match `anvilml.toml`'s values (`host = "127.0.0.1"`, `port = 8488`).

**Gap between design doc and source:** None relevant to this task. The design doc's config precedence chain (§15) is partially implemented — CLI flags are wired, but the `--config` flag's file-loading logic has not been implemented yet. This does not affect the Runnable Proof since the binary runs on defaults.

## Resolved Dependencies

None. This task performs no source changes and introduces no dependencies. It only runs existing build and network commands.

## Approach

1. **Build the release binary.** Run `cargo build --release -p anvilml`. This compiles the full workspace with all feature flags forwarded through the dependency graph. The binary is produced at `target/release/anvilml`.

2. **Launch the binary in the background.** Run `./target/release/anvilml &` to start the server on `127.0.0.1:8488` (the CLI defaults). The `main.rs` code binds the TCP listener, logs the address via `tracing::info!`, and enters the `tokio::select!` loop waiting for HTTP requests or a shutdown signal.

3. **Wait for the server to be ready.** Run `sleep 1` to allow the server to bind the port and start accepting connections.

4. **Send the health check request.** Run `curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8488/health`. This sends an HTTP GET to the health endpoint and prints only the HTTP status code. The expected output is `200`.

5. **Record the output.** Capture the literal `200` response. This is the Runnable Proof transcript for Phase 1.

6. **Clean up.** Run `kill %1` to terminate the backgrounded server process.

No source files are created or modified. No new dependencies are introduced. No test changes are required — the existing in-process test (`health_tests.rs`) already covers the route logic; this task exercises the same code path through a real TCP connection and real HTTP request.

## Public API Surface

None. No source code changes are made in this task.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| No changes | — | This task performs no source modifications |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| (existing) `crates/anvilml-server/tests/health_tests.rs` | `test_health_returns_200` | The `GET /health` route returns `200 OK` via in-process router dispatch | `cargo test -p anvilml-server --test health_tests` exits 0 |
| (live) — | `test_health_live_binary` | The release binary binds TCP on `127.0.0.1:8488` and answers `GET /health` with `200` over a real socket | The acceptance command in the task's acceptance criteria prints `200` |

## CI Impact

No CI changes required. This task introduces no new files, no new tests, and no new build configurations.

## Platform Considerations

None identified. The acceptance command (`curl` + background process + `kill %1`) works identically on Linux and Windows (WSL2). The server binds `127.0.0.1` which is the loopback address on all platforms. No `#[cfg]` guards are needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Port 8488 is already in use from a prior session | Low | Medium | The `kill %1` in step 6 is the cleanup; if the port is already bound, `TcpListener::bind` will fail with an `AddrInUse` error, and `main.rs` calls `.unwrap()` which will panic. The ACT agent should check for stale processes first (`lsof -i :8488`) and kill any existing process on that port before launching. |
| Release build takes a long time or fails due to stale artifacts | Medium | Medium | The acceptance command uses `cargo build --release` which handles incremental compilation. If it fails, the ACT agent should run `cargo clean -p anvilml` and retry, or `cargo clean` if needed. |
| curl is not installed in the environment | Low | Low | `curl` is a standard tool on Linux and WSL2. If absent, the ACT agent should install it (`apt-get install -y curl`) before running the acceptance command. |

## Acceptance Criteria

- [ ] `cargo build --release -p anvilml` exits 0
- [ ] `./target/release/anvilml & sleep 1 && curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8488/health` outputs `200`
- [ ] `kill %1` terminates the backgrounded process
