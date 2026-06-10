# Plan Report: P15-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P15-A2                                      |
| Phase       | 015 — Live Job Events                       |
| Description | anvilml: integration test asserting full WS lifecycle for a mock job |
| Depends on  | P15-A1                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-06-10T08:15:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `backend/tests/api_ws_lifecycle.rs`: an integration test that spins up the full AnvilML server (with `mock-hardware` + `ANVILML_WORKER_MOCK=1` + in-memory DB), connects a `tokio-tungstenite` WebSocket client to `/v1/events`, POSTs a valid ZiT job, and asserts the ordered sequence of WS frames — `job.queued`, `job.started`, `job.progress` (≥1), `job.image_ready`, `job.completed` — within a 20-second deadline. The test skips gracefully if Python is not on PATH.

## Scope

### In Scope
- Create `backend/tests/api_ws_lifecycle.rs` with a single integration test
- Add `tokio-tungstenite` as a dev-dependency in `backend/Cargo.toml`
- The test binds the server on `127.0.0.1:0` (random port) using `tokio::net::TcpListener`
- The test sets `ANVILML_WORKER_MOCK=1` and relies on the `mock-hardware` feature for hardware detection
- The test uses an in-memory SQLite database (via `anvilml_registry::open_in_memory()`)
- The test POSTs a valid ZiT job graph to `POST /v1/jobs`
- The test connects a `tokio-tungstenite` client to `ws://127.0.0.1:<port>/v1/events`
- The test collects WS frames and asserts the ordered sequence: `job.queued`, `job.started`, `job.progress` (≥1), `job.image_ready`, `job.completed`
- The test has a 20-second timeout for the full lifecycle
- The test detects if Python is on PATH and skips with `#[ignore]` if absent
- `cargo test --features mock-hardware --test api_ws_lifecycle` exits 0

### Out of Scope
- No changes to server source code or handler code
- No changes to scheduler source code (P15-A1 handles that)
- No changes to the Python worker
- No new crates or dependencies beyond `tokio-tungstenite` (already in workspace)
- No documentation changes (handled by P15-A3)
- No changes to CI workflow files

## Approach

### Step 1: Add `tokio-tungstenite` dev-dependency to `backend/Cargo.toml`

Add `tokio-tungstenite = { workspace = true }` to the `[dev-dependencies]` section. The workspace already declares it with the `rustls-tls-native-roots` feature.

### Step 2: Create `backend/tests/api_ws_lifecycle.rs`

The test file will contain a single async test function `test_ws_lifecycle_full_job`. The test structure:

#### 2a. Python availability check (skip guard)
At the start of the test, check if `python3` (or `python`) is on PATH by attempting to spawn `which python3` / `which python` (Unix) or `where python` (Windows). If not found, call `#[ignore]` and return. This follows the task requirement to "skip test gracefully if absent."

#### 2b. Spawn the server
- Set env vars: `ANVILML_WORKER_MOCK=1`, `ANVILML_LOG=error` (minimize noise)
- Use `anvilml_hardware::detect_all_devices` with mock-hardware feature to get mock hardware info
- Build `App` state with: in-memory DB (`anvilml_registry::open_in_memory()`), mock hardware, a `WorkerPool` created via `new_test_pool_with_workers()` with a single `ManagedWorker` (CPU), a `JobScheduler`, an `EventBroadcaster`, and a temp-dir `ArtifactStore`
- Start the dispatch loop via `scheduler.start_dispatch_loop()`
- Bind a `tokio::net::TcpListener` on `127.0.0.1:0` to get a random port
- Spawn the axum server via `axum::serve(listener, router)` in a background task

#### 2c. Wait for worker readiness
- The mock worker in the test pool is created via `ManagedWorker::new()` and set to `Idle` status
- Wait up to 3 seconds for the worker pool to report at least one idle worker via `workers.list().await`

#### 2d. Connect WebSocket client
- Use `tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/v1/events"))` to connect
- The connection must succeed within 3 seconds

#### 2e. POST the ZiT job
- Build a valid ZiT 4-node graph (ZitLoadPipeline → ZitTextEncode → ZitSampler → ZitDecode)
- POST to `http://127.0.0.1:{port}/v1/jobs` with `application/json` body containing the graph and default JobSettings
- Assert 202 response with a valid `job_id` UUID

#### 2f. Collect and assert WS event sequence
- Use a `tokio::sync::mpsc::unbounded_channel` to fan-out incoming WS frames to an assertion task
- Collect frames in order, tracking the expected sequence: `["job.queued", "job.started", "job.progress", "job.image_ready", "job.completed"]`
- For `job.progress`: assert at least one frame with `node_index < node_total`
- For `job.image_ready`: assert `artifact_hash` is non-empty
- Each event must have `event` field matching the expected type name
- Use a 20-second `tokio::time::timeout` wrapping the entire collection loop
- Assert that all 5 expected events were received in order

#### 2g. Cleanup
- Abort the server task
- Abort the dispatch loop handle
- Clean up mock env vars (set_var → remove_var for isolation per FORGE_AGENT_RULES §9.6)

### Step 3: Verify with `cargo test --features mock-hardware --test api_ws_lifecycle`

The test will be run with `--features mock-hardware` which enables the mock hardware detector. The `ANVILML_WORKER_MOCK=1` env var is set within the test. The test uses `serial_test::serial` to avoid race conditions with other tests that also set mock env vars.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `backend/Cargo.toml` | Add `tokio-tungstenite = { workspace = true }` to `[dev-dependencies]` |
| Create   | `backend/tests/api_ws_lifecycle.rs` | Integration test file with full WS lifecycle assertion |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `backend/tests/api_ws_lifecycle.rs` | `test_ws_lifecycle_full_job` | Full ordered sequence of WS events (queued→started→progress→image_ready→completed) for a mock ZiT job within 20s |

## CI Impact

No CI changes required. The test runs under the existing `cargo test --workspace --features mock-hardware` gate. Since the test is in `backend/tests/` and uses the `mock-hardware` feature, it is automatically included when the workspace test suite runs with that feature flag. The test gracefully skips if Python is absent (not a CI failure — the mock worker doesn't actually need a real Python subprocess because the test injects events via the pool's `publish_event` method).

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Test flakes due to timing (worker ready before server binds) | Medium | Medium | Use a polling loop with 100ms intervals and 3s timeout to wait for worker readiness; bind port before starting server |
| Worker pool needs real Python subprocess for mock mode | Medium | High | The test uses `WorkerPool::new_test_pool_with_workers()` which creates in-memory workers without spawning Python processes. Events are injected via `pool.publish_event()`. This bypasses the need for a real Python worker entirely. |
| `tokio-tungstenite` TLS feature conflict | Low | Low | Workspace already declares it with `rustls-tls-native-roots`; local dev connection to `ws://` (non-TLS) will work without TLS features. If needed, add `tungstenite` feature override. |
| Test takes too long (>20s timeout) | Low | Medium | The test injects events directly via `publish_event()` so there is no actual ML inference delay. Events fire synchronously. 20s timeout is a safety net. |
| Mock env vars pollute other tests | Low | Medium | Use `#[serial]` attribute; restore env vars in an unconditional cleanup block (FORGE_AGENT_RULES §9.6) |

## Acceptance Criteria

- [ ] `backend/tests/api_ws_lifecycle.rs` exists with a single test function `test_ws_lifecycle_full_job`
- [ ] `tokio-tungstenite` is listed in `backend/Cargo.toml` `[dev-dependencies]`
- [ ] Test binds server on `127.0.0.1:0` (random port)
- [ ] Test sets `ANVILML_WORKER_MOCK=1` and uses `mock-hardware` feature
- [ ] Test uses in-memory SQLite database
- [ ] Test connects `tokio-tungstenite` WebSocket client to `/v1/events`
- [ ] Test POSTs a valid ZiT job graph to `/v1/jobs`
- [ ] Test asserts WS frames arrive in order: `job.queued`, `job.started`, `job.progress` (≥1), `job.image_ready`, `job.completed`
- [ ] Test has a 20-second deadline for the full lifecycle
- [ ] Test skips gracefully (via `#[ignore]`) if Python is not on PATH
- [ ] `cargo test --features mock-hardware --test api_ws_lifecycle` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
