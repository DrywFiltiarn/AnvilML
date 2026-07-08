# Plan Report: P15-D1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P15-D1                                      |
| Phase       | 015 — Artifact Storage Wiring               |
| Description | Runnable Proof: PassThrough-derived job artifact retrievable via HTTP |
| Depends on  | P15-A1, P15-B1, P15-B2, P15-C1             |
| Project     | anvilml                                     |
| Planned at  | 2026-07-08T22:58:00Z                        |
| Attempt     | 1                                           |

## Objective

Produce Phase 15's Runnable Proof: build the AnvilML server binary with `--features mock-hardware`, start it, and demonstrate that `GET /v1/artifacts` returns HTTP 200 with an empty JSON array `[]`. This proves the artifact listing endpoint is live and wired end-to-end through `AppState` → `ArtifactStore` → SQLite, even though no artifact-producing node (image-generating node chain) exists yet — `PassThrough` is the only node in the project and emits no `ImageReady` events. The first populated response requires a real image-producing node chain (`LoadModel` → `Sampler` → `VaeDecode` → `SaveImage`), which arrives in later architecture-loading phases.

## Scope

### In Scope
- Build the release binary with `--features mock-hardware`.
- Start the server, wait for it to be ready.
- Send `GET /v1/artifacts` and verify HTTP 200.
- Send `GET /v1/artifacts` and verify the response body is `[]`.
- Stop the background server process.
- Record the literal terminal output in the implementation report.

### Out of Scope
None. `defers_to (from JSON): []` — this task implements its full scope with no deferrals.

## Existing Codebase Assessment

Three prior tasks in Phase 15 (P15-A1, P15-B1, P15-B2) plus P15-C1 have already built the complete artifact infrastructure:

**(a) What exists:**
- `AppState` (`crates/anvilml-server/src/state.rs`) holds `artifact_store: Arc<ArtifactStore>`, constructed in `backend/src/main.rs` with the config's `artifact_dir` and the shared `SqlitePool`.
- `GET /v1/artifacts` (list) and `GET /v1/artifacts/{hash}` (retrieve) are wired in `crates/anvilml-server/src/lib.rs` and implemented in `crates/anvilml-server/src/handlers/artifacts.rs` as thin delegations to `ArtifactStore::list()` and `ArtifactStore::get()`.
- `event_loop.rs` (`crates/anvilml-scheduler/src/event_loop.rs`) contains `handle_image_ready()` which decodes base64, constructs `ArtifactMeta`, and calls `artifact_store.save()`.
- The `JobScheduler` constructor accepts `Arc<ArtifactStore>` and passes it to the event loop.
- 8 integration tests exist in `crates/anvilml-server/tests/artifacts_tests.rs` covering empty-store, populated-store, job_id filter, JSON shape, content-type header, and byte-for-byte retrieval.
- 4+ unit tests exist in `crates/anvilml-scheduler/tests/event_loop_tests.rs` for `handle_image_ready()`.

**(b) Established patterns:**
- Handlers are thin delegations with zero business logic (per ANVILML_DESIGN.md §3.3).
- `#[tracing::instrument]` is used on all public async functions.
- Error types use `AnvilError` enum with `IntoResponse` impl for axum.
- Test helpers (`make_test_state()`, `save_artifact()`) live in the test crate and use in-memory SQLite.

**(c) No gap between design doc and current source.** All Phase 15 deliverables are compiled and tested. The only missing piece is the Runnable Proof execution.

## Resolved Dependencies

None. This task introduces no new dependencies — it runs the already-built binary with existing crate versions.

| Type | Name | Version verified | MCP source | Feature flags confirmed |
|------|------|-----------------|------------|------------------------|
| *(none — no new dependencies)* | | | | |

## Approach

1. **Build the release binary.**
   ```bash
   cargo build --release -p anvilml --features mock-hardware
   ```
   This compiles the entire workspace with mock hardware enabled, producing `./target/release/anvilml`. The `mock-hardware` feature replaces GPU detection with `MockDetector` and causes `build_worker_env()` to inject `ANVILML_WORKER_MOCK=1` into spawned subprocesses.

2. **Start the server in the background.**
   ```bash
   ./target/release/anvilml &
   ```
   The server binds to `127.0.0.1:8488` (default from `anvilml.toml`). It initializes the database, loads device capabilities seed, detects mock hardware, and starts the HTTP router.

3. **Wait for the server to be ready.**
   ```bash
   sleep 1
   ```
   One second is sufficient for the server to complete startup (database init, seed loading, hardware detection, worker spawning, and HTTP listener binding). This is the same wait used in the acceptance criterion.

4. **Verify the HTTP status code.**
   ```bash
   curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8488/v1/artifacts
   ```
   Expected output: `200`. This confirms the endpoint is live, the router correctly matches `/v1/artifacts`, the `list_artifacts()` handler receives the request, delegates to `artifact_store.list(None)`, and returns a successful response.

5. **Verify the response body is an empty array.**
   ```bash
   curl -s http://127.0.0.1:8488/v1/artifacts
   ```
   Expected output: `[]`. This confirms the artifact store's `list()` method returns an empty vector when no artifacts exist in the database, and the handler correctly serialises it as a JSON array (not `null`).

6. **Stop the background server.**
   ```bash
   kill %1
   ```
   The server's graceful shutdown sequence (per `backend/src/main.rs`) aborts the dispatch loop, reclaims `WorkerPool` ownership, and calls `shutdown_all()` on each worker.

7. **Record the literal terminal output** from steps 4 and 5 in the implementation report's `## Runnable Proof Transcript` section.

## Public API Surface

No new public items. This task does not modify any source files. The existing public API surface exercised by this proof:

- `GET /v1/artifacts` → `200 [ArtifactMeta, ...]` (handler: `handlers::artifacts::list_artifacts`)
- `GET /v1/artifacts/{hash}` → `200 image/png` or `404` (handler: `handlers::artifacts::get_artifact`)
- `AppState.artifact_store: Arc<ArtifactStore>` (state field)
- `ArtifactStore::list(job_id: Option<Uuid>) -> impl Future<Output = Result<Vec<ArtifactMeta>, AnvilError>>`
- `ArtifactStore::get(hash: &str) -> impl Future<Output = Result<Option<Vec<u8>>, AnvilError>>`

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| (none) | — | This task modifies no source files. All Phase 15 infrastructure is already in place. |

## Tests

No new tests are written in this task. The phase's test coverage for the artifact endpoints already exists:

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|--------------------|
| `crates/anvilml-server/tests/artifacts_tests.rs` | `test_list_artifacts_empty_store_returns_200_empty_array` | GET /v1/artifacts returns 200 with `[]` when store is empty | `cargo test -p anvilml-server --test artifacts_tests -- test_list_artifacts_empty_store_returns_200_empty_array` |
| `crates/anvilml-server/tests/artifacts_tests.rs` | `test_list_artifacts_populated_returns_all` | GET /v1/artifacts returns all artifacts when store is populated | Same as above |
| `crates/anvilml-server/tests/artifacts_tests.rs` | `test_list_artifacts_job_id_filter_returns_matching` | GET /v1/artifacts?job_id=<uuid> filters correctly | Same as above |
| `crates/anvilml-server/tests/artifacts_tests.rs` | `test_list_artifacts_json_shape` | Response body has correct JSON shape with all ArtifactMeta fields | Same as above |
| `crates/anvilml-server/tests/artifacts_tests.rs` | `test_get_artifact_existing_hash_returns_200` | GET /v1/artifacts/{hash} returns 200 for saved artifact | Same as above |
| `crates/anvilml-server/tests/artifacts_tests.rs` | `test_get_artifact_unknown_hash_returns_404` | GET /v1/artifacts/{hash} returns 404 for unknown hash | Same as above |
| `crates/anvilml-server/tests/artifacts_tests.rs` | `test_get_artifact_byte_for_byte_match` | GET /v1/artifacts/{hash} returns identical bytes to what was saved | Same as above |
| `crates/anvilml-server/tests/artifacts_tests.rs` | `test_get_artifact_content_type_header` | Content-Type header is exactly `image/png` | Same as above |

## CI Impact

No CI changes required. This task does not modify any CI workflow files, test configurations, or build scripts. The existing CI jobs (rust-linux, rust-windows, config-drift) already exercise the artifact endpoints through the integration tests in `artifacts_tests.rs`.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The server binary is platform-neutral for this proof — it binds to localhost, listens on a TCP port, and responds to HTTP requests identically on Linux and Windows. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are exercised by this proof.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Port 8488 is already in use from a previous run | Low | Medium | The `kill %1` step in step 6 handles cleanup. If the port is occupied, the server will fail to bind and exit with an error — the proof fails cleanly with a visible error message rather than hanging. |
| Server startup takes longer than 1 second | Low | Medium | If curl returns a connection error, increase the sleep to 2 seconds. The server's startup sequence (database init + seed loading + hardware detection + worker spawning + HTTP bind) is deterministic and typically completes in under 500ms on the agent VM. |
| Mock-hardware binary produces unexpected log output that interferes with curl output | Very Low | Low | The server writes logs to stderr, not stdout. `curl` captures only the HTTP response body on stdout, so log output does not interfere. |

## Acceptance Criteria

- [ ] `cargo build --release -p anvilml --features mock-hardware` exits 0
- [ ] `./target/release/anvilml & sleep 1 && curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8488/v1/artifacts` prints `200`
- [ ] `curl -s http://127.0.0.1:8488/v1/artifacts` prints `[]`
- [ ] `kill %1` terminates the background server process

## Phase Deliverable Audit

This is the phase-closing task (last entry in `tasks_phase015.json` array). The following audits were run per FORGE_AGENT_RULES.md §9a, §9a.1, and §9a.2:

**§9a — defers_to coverage:** No tasks in Phase 15 have non-empty `defers_to` fields. No `defers_to:` comment markers were found in any phase 15 source files.
```bash
grep -rn "defers_to: " crates/anvilml-server/src/handlers/artifacts.rs crates/anvilml-server/src/state.rs crates/anvilml-scheduler/src/event_loop.rs crates/anvilml-scheduler/src/scheduler.rs crates/anvilml-server/src/lib.rs backend/src/main.rs
# Result: no matches (empty output)
```

**§9a.1 — Unmarked-stub sweep:**
```bash
grep -rn "NotImplementedError\|unimplemented!\|todo!\|# TODO\|// TODO" \
  crates/anvilml-server/src/handlers/artifacts.rs \
  crates/anvilml-server/src/state.rs \
  crates/anvilml-scheduler/src/event_loop.rs \
  crates/anvilml-scheduler/src/scheduler.rs \
  crates/anvilml-server/src/lib.rs \
  backend/src/main.rs
# Result: 0 findings
```

**§9a.2 — Dual-mode parity-marker sweep:** The project defines `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` markers (ANVILML_DESIGN.md §10.6), but these apply only to node `execute()` and arch-module `load()`/`sample()`/`decode()`/`compute_latent_shape()` functions in `worker/nodes/`. Phase 15 does not modify any files under `worker/nodes/`. The server crate files (`handlers/artifacts.rs`, `state.rs`, `event_loop.rs`, `scheduler.rs`, `lib.rs`, `main.rs`) are not in scope for the parity-marker convention.
```bash
grep -L "REAL_PATH_VERIFIED:" crates/anvilml-server/src/handlers/artifacts.rs crates/anvilml-server/src/state.rs crates/anvilml-scheduler/src/event_loop.rs crates/anvilml-scheduler/src/scheduler.rs crates/anvilml-server/src/lib.rs backend/src/main.rs
# Result: lists all 6 files (markers not expected in server code)
grep -L "MOCK_PATH_VERIFIED:" crates/anvilml-server/src/handlers/artifacts.rs crates/anvilml-server/src/state.rs crates/anvilml-scheduler/src/event_loop.rs crates/anvilml-scheduler/src/scheduler.rs crates/anvilml-server/src/lib.rs backend/src/main.rs
# Result: lists all 6 files (markers not expected in server code)
```
These results are expected and non-finding: the parity-marker convention does not apply to these files.

**Summary:** Phase 15 passes all three audit checks. No blockers.
