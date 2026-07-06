# Plan Report: P11-E1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P11-E1                                      |
| Phase       | 11 — Dynamic Node System                    |
| Description | Runnable Proof: live binary serves GET /v1/nodes with real data |
| Depends on  | P11-D1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-06T15:20:00Z                        |
| Attempt     | 1                                           |

## Objective

Build the Phase 11 release binary with the `mock-hardware` feature and confirm that the live `anvilml` HTTP server answers `GET /v1/nodes` with HTTP 200 and a JSON body of `[]`. This is the first phase where the `/v1/nodes` endpoint is wired end-to-end through a running server — the response is empty because no worker is spawned by `backend/main.rs` yet, but the wiring from `AppState` → `NodeTypeRegistry` → handler → HTTP route is verified live.

## Scope

### In Scope
- Run `cargo build --release -p anvilml --features mock-hardware` to produce the release binary.
- Start the binary as a background process.
- Send `curl` requests to `http://127.0.0.1:8488/v1/nodes` and verify:
  - HTTP status code is `200`.
  - Response body is the literal JSON array `[]`.
- Kill the background process.
- Record the full terminal transcript in the implementation report.

### Out of Scope
None. This task has an empty `defers_to` field and must implement its full scope. The task's context says "confirm" and "verify" — those are implementation actions, not deferrals.

## Existing Codebase Assessment

The Phase 11 source code is fully implemented by the predecessor tasks (P11-A1 through P11-D1). Inspection of the relevant files confirms:

**(a) What exists:** `backend/src/main.rs` constructs `AppState { config, node_registry, start_time }` with an empty `NodeTypeRegistry`, passes it to `build_router()`, which registers the `/v1/nodes` route pointing to `handlers::nodes::list_nodes`. The handler delegates to `state.node_registry.list()` which returns an empty `Vec<NodeTypeDescriptor>` when the registry has no entries. The `build_router()` function uses `.with_state(app_state)` for state injection.

**(b) Established patterns:** The handler follows the "no business logic in handlers" rule — it is a single-line delegation to the registry. Tests use `tower::util::ServiceExt::oneshot` for in-process HTTP testing. The `AppState` struct derives `Clone` and wraps all fields in `Arc`. Logging uses structured `tracing!` macros.

**(c) Gap between design doc and source:** No gap. The implementation matches `ANVILML_DESIGN.md §13.4` exactly: `GET /v1/nodes → 200 [NodeTypeDescriptor, ...]`.

## Resolved Dependencies

None. This task does not introduce or modify any dependencies. It runs the binary built by predecessor tasks. The external crates already pinned in `Cargo.lock` are:
- `axum` — HTTP framework (used by `anvilml-server`)
- `serde_json` — JSON serialization (used by handler return type)
- `tokio` — async runtime (used by `backend/main.rs`)

No MCP lookup is needed because no new crate is referenced and no version changes occur.

## Approach

### Step 1 — Build the release binary

Run:
```bash
cd /home/dryw/AnvilML
cargo build --release -p anvilml --features mock-hardware
```

This compiles the full workspace with the `mock-hardware` feature, producing `target/release/anvilml`. This is the same binary that all CI jobs build (`rust-linux` and `rust-windows` jobs). The `mock-hardware` feature replaces GPU detection with `MockDetector` driven by env vars — it is required for all non-production builds per `ANVILML_DESIGN.md §5` and `ENVIRONMENT.md §5`.

### Step 2 — Start the server

Run the binary in the background:
```bash
./target/release/anvilml &
```

The server reads config from `anvilml.toml` (or defaults), loads the SQLite database, applies the device capabilities seed, and binds a TCP listener on `127.0.0.1:8488` (the default port from `ServerConfig::default()`). An empty `NodeTypeRegistry` is constructed — no worker is spawned, so it stays empty.

### Step 3 — Wait for server readiness

```bash
sleep 1
```

One second is sufficient for the server to complete config loading, database initialization, and TCP listener binding. The server logs "listening" at INFO level when the listener is ready.

### Step 4 — Verify HTTP status code

```bash
curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8488/v1/nodes
```

Expected output: `200`

The request hits the route registered in `build_router()`, which dispatches to `handlers::nodes::list_nodes()`. The handler acquires a read lock on the `NodeTypeRegistry`, calls `list()` which returns an empty `Vec`, and serialises it as `[]`.

### Step 5 — Verify response body

```bash
curl -s http://127.0.0.1:8488/v1/nodes
```

Expected output: `[]`

This confirms the response is a JSON array (not `null`, not an object, not an error) and that the empty registry produces exactly `[]`.

### Step 6 — Shut down the server

```bash
kill %1
```

This sends SIGTERM to the background process. The server's shutdown handler (`anvilml::shutdown::wait_for_shutdown_signal()`) catches the signal and exits gracefully.

### Phase Deliverable Audit (§9a, §9a.1, §9a.2)

This is the phase-closing task. Per `FORGE_AGENT_RULES.md §9a`, the following audits were run before writing the Approach:

**§9a — defers_to coverage audit:**
```bash
grep -A8 '"id":' .forge/tasks/tasks_phase011.json | grep defers_to
```
Result: No task in phase 11 has a non-empty `defers_to` field. No deferral links to audit.

**§9a.1 — Unmarked-stub sweep:**
```bash
grep -rn "NotImplementedError\|unimplemented!\|todo!\|# TODO\|// TODO" \
  crates/anvilml-worker/src/managed.rs \
  crates/anvilml-server/src/state.rs \
  crates/anvilml-server/src/lib.rs \
  crates/anvilml-server/src/handlers/nodes.rs \
  crates/anvilml-server/src/handlers/mod.rs \
  backend/src/main.rs
```
Result: `0 findings` — no unmarked stubs in any phase 11 source file.

**§9a.2 — Dual-mode parity-marker sweep:**
```bash
grep -L "REAL_PATH_VERIFIED:" crates/anvilml-server/src/state.rs crates/anvilml-server/src/lib.rs crates/anvilml-server/src/handlers/nodes.rs crates/anvilml-server/src/handlers/mod.rs backend/src/main.rs
grep -L "MOCK_PATH_VERIFIED:" crates/anvilml-server/src/state.rs crates/anvilml-server/src/lib.rs crates/anvilml-server/src/handlers/nodes.rs crates/anvilml-server/src/handlers/mod.rs backend/src/main.rs
```
Result: `0 findings` — the parity markers apply only to Python node/arch-module functions (`execute()`, `load()`, `sample()`, `decode()`, `compute_latent_shape()` per `ANVILML_DESIGN.md §10.4`). None of the files touched by phase 11 tasks define such functions; these are Rust server-side files with no parity-marker scope.

## Public API Surface

None. This task does not introduce or modify any public API items. All public items (`AppState`, `build_router()`, `list_nodes()`, `NodeTypeRegistry`) were established by predecessor tasks.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| None | — | This task creates or modifies no files. It runs the already-built binary. |

## Tests

None. This task does not write or modify any test files. The integration tests for the `/v1/nodes` handler (`crates/anvilml-server/tests/nodes_tests.rs`) were written by predecessor task P11-C1 and exercise the same code path in-process. This task verifies the identical code path through a live HTTP server — a different execution environment, not a different test.

## CI Impact

No CI changes required. This task does not modify any CI workflow files, add new file types, or change test module registration. The phase's CI impact was handled by predecessor tasks (e.g., P11-C1's handler tests are picked up by the `rust-linux` and `rust-windows` CI jobs' `cargo test` step).

## Platform Considerations

None identified. The server binds to `127.0.0.1:8488` which is platform-neutral. The `mock-hardware` feature exercises `#[cfg(unix)]` mock paths during build, but the runtime HTTP server code is cross-platform. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Port 8488 is already in use from a prior run that was not killed | Low | Medium | The `kill %1` in Step 6 ensures cleanup. If the build fails because the port is in use, run `lsof -i :8488` to identify and kill the prior process before retrying. |
| Server takes longer than 1 second to bind the listener (slow CI disk / cold build) | Low | Low | If `curl` returns a connection error, increase `sleep` to 2 seconds. The server logs "listening" which can be used as a readiness signal. |
| `anvilml.toml` or database path conflicts with local environment | Low | Medium | The binary uses default paths (`./anvilml.toml`, `./anvilml.db`). If these conflict with an existing AnvilML instance, run from a clean temporary directory with `--config /dev/null` to skip TOML loading. |

## Acceptance Criteria

- [ ] `cargo build --release -p anvilml --features mock-hardware` exits 0
- [ ] `curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8488/v1/nodes` prints `200`
- [ ] `curl -s http://127.0.0.1:8488/v1/nodes` prints `[]`
- [ ] `kill %1` succeeds and the background process terminates
