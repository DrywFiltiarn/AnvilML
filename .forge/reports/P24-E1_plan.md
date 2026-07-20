# Plan Report: P24-E1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P24-E1                                            |
| Phase       | 24 — Generic Conditioning/Sampling/Decode Nodes   |
| Description | anvilml-server: real end-to-end ZiT generation graph via POST /v1/jobs |
| Depends on  | P24-A2, P24-C2, P24-D3                            |
| Project     | anvilml                                            |
| Planned at  | 2026-07-19T23:15:00Z                              |
| Attempt     | 1                                                  |

## Objective

Create >=3 integration tests (Python) that submit ANVILML_DESIGN.md Appendix B.2's exact ZiT generation graph (`LoadModel`+`LoadVae`+`LoadClip`+`EmptyLatent`+`ClipTextEncode`+`Sampler`+`VaeDecode`+`SaveImage`) via `POST /v1/jobs` against a live server instance, verify the job transitions to `Completed` state, and retrieve a real, retrievable PNG artifact via `GET /v1/artifacts/:hash`. This is the first time the **generic node layer** (not direct arch-module calls) produces a real image through the actual HTTP → scheduler → dispatch → execute → artifact pipeline.

## Scope

### In Scope
- New Python integration test file: `worker/tests/test_e2e_full_graph.py`
  - Test 1: `test_full_graph_mock_mode` — submits the full Appendix B.2 graph in mock mode, verifies job reaches `Completed`, retrieves artifact
  - Test 2: `test_full_graph_real_mode` — same graph in real mode (fixture checkpoints), verifies job reaches `Completed`, retrieves artifact with dimension check
  - Test 3: `test_full_graph_invalid_graph_returns_400` — submits a structurally invalid graph (unknown node type), verifies `400 Bad Request`
- No new production source files — only test code
- Tests spawn the Rust binary as a subprocess, connect a mock worker via ZeroMQ, and exercise the full HTTP + dispatch pipeline
- Updates `docs/TESTS.md` with entries for the new tests

### Out of Scope
- The Runnable Proof (P24-F1) — a separate phase-closing task that runs the same graph against a real (non-mock-hardware) binary
- No new production code, handlers, or API changes
- No changes to existing test files or source files

## Existing Codebase Assessment

The codebase has a complete generic node system built across Phases 19–24:
- **Rust server** (`anvilml-server`): axum HTTP handlers for `POST /v1/jobs`, `GET /v1/jobs/:id`, `GET /v1/artifacts/:hash`, `POST /v1/models/rescan`
- **Scheduler** (`anvilml-scheduler`): job queue, graph validation, dispatch loop with `dispatch_one()`, event loop with `spawn_event_loop()` for `ImageReady` artifact persistence
- **Python worker** (`worker/`): generic node registry with all 8 MVP nodes registered, arch modules for ZiT diffusion/Qwen3 CLIP/ZiT VAE
- **Existing e2e test** (`worker/tests/test_e2e_zit_pipeline.py`): tests the full load+sample+decode chain via direct arch-module calls (not through the generic node layer or HTTP API) — this is P23-F1's scope, not P24-E1's
- **Existing Rust integration tests** (`crates/anvilml-server/tests/jobs_tests.rs`): use in-process `ServiceExt::oneshot()` HTTP calls with stub state — do NOT exercise the dispatch pipeline or worker execution
- **Fixture checkpoints**: `zit_tiny.safetensors`, `zit_vae_tiny.safetensors`, `qwen3_tiny.safetensors` — tiny synthetic safetensors under `worker/tests/fixtures/`

The established patterns to follow:
- Python tests use `pytest` with `@pytest.mark.real_mode` for torch-dependent tests
- Subprocess spawning uses `subprocess.Popen` with explicit `timeout=` on all blocking calls (FORGE_AGENT_RULES §5.12, ENVIRONMENT.md §11.5)
- ZeroMQ socket operations use `setsockopt(zmq.RCVTIMEO, 5000)` before blocking recv (ENVIRONMENT.md §11.5)
- Tests that spawn subprocesses capture and surface stderr on timeout failure (ENVIRONMENT.md §11.5)

Gap between design doc and current source: The existing `test_e2e_zit_pipeline.py` tests the arch modules directly, not the generic node layer. P24-E1 requires the full HTTP API → scheduler → dispatch → generic node execution pipeline, which is a different code path that the existing test does not cover.

## Resolved Dependencies

None. This task introduces no new external dependencies. It uses existing packages already in `worker/requirements/base.txt` (`pytest`, `pyzmq`, `httpx`/`requests`).

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| (none) | (none)  | N/A             | N/A            | N/A                    |

## Approach

### Step 1: Create `worker/tests/test_e2e_full_graph.py`

Create a new Python integration test file with the following structure:

**Module-level guarded imports** (following the pattern in `test_e2e_zit_pipeline.py`):
```python
try:
    import torch
except ImportError:
    torch = None
```

**Helper function `_make_mock_worker()`**: Creates a minimal mock Python worker process that:
1. Connects a ZeroMQ DEALER socket to the server's ROUTER port (read from `ANVILML_IPC_PORT` env var injected by the test)
2. Sends a `Ready` event with all required node types registered (LoadModel, LoadVae, LoadClip, EmptyLatent, ClipTextEncode, Sampler, VaeDecode, SaveImage)
3. Enters a dispatch loop that receives `Execute` messages and returns `Completed` events with a mock image payload

This mock worker follows the same pattern as the real worker's startup sequence (ENVIRONMENT.md §5) but returns sentinel outputs instead of running torch inference.

**Helper function `_start_server()`**: Spawns the Rust binary:
```bash
cargo build --release -p anvilml  # or use pre-built binary
```
Actually, for test efficiency, the test should use the pre-built binary from `target/release/anvilml` and pass `--config anvilml.toml --model-dirs worker/tests/fixtures`. The server binds to `127.0.0.1:8488` by default.

**Test 1: `test_full_graph_mock_mode`** (`@pytest.mark.serial`):
1. Start the Rust server subprocess with `ANVILML_FORCE_WORKER_MOCK=1` in the environment
2. Wait for the server to be ready (poll `GET /health` with timeout)
3. Start the mock worker subprocess connecting to the server's IPC port
4. Wait for the worker to send `Ready` (poll `GET /v1/nodes` until all 8 node types appear)
5. Submit the full Appendix B.2 graph via `POST /v1/jobs` with fixture model IDs
6. Poll `GET /v1/jobs/{job_id}` every 500ms until `status == "completed"` (timeout: 60s)
7. Retrieve the artifact via `GET /v1/artifacts/{hash}`
8. Assert the response is a valid PNG (check magic bytes `89 50 4e 47`)
9. Teardown: terminate server and worker subprocesses, restore env vars

**Test 2: `test_full_graph_real_mode`** (`@pytest.mark.real_mode` + `@pytest.mark.serial`):
1. Same setup as Test 1 but with a real Python worker (no `ANVILML_FORCE_WORKER_MOCK`)
2. The real worker loads fixture checkpoints and executes the full generic node graph
3. After job completion, retrieve the artifact and assert it is a valid PNG with dimensions matching the requested 64×64
4. This test requires torch (real_mode marker ensures environment)

**Test 3: `test_full_graph_invalid_graph_returns_400`** (`@pytest.mark.serial`):
1. Start the server (same as Test 1)
2. Submit a graph with an unknown node type (e.g., `"type": "NonExistentNode"`)
3. Assert response status is `400 Bad Request`
4. Assert the response body contains a validation error message
5. Teardown: terminate server subprocess

**Key implementation details:**
- The mock worker sends a `Completed` event with a base64-encoded 64×64 black PNG (following ENVIRONMENT.md §11.5's pattern for timeout-safe IPC)
- All subprocess calls use explicit `timeout=` (FORGE_AGENT_RULES §5.12)
- ZeroMQ `RCVTIMEO` is set to 5000ms before any blocking recv
- On timeout, subprocess stderr is captured and included in the failure message
- Tests use `@pytest.mark.serial` because they mutate process-global state (server/worker subprocesses sharing ports)

### Step 2: Update `docs/TESTS.md`

Add entries for the three new tests following the existing format in `docs/TESTS.md`:
- Test name, what it verifies, Mode (mock/real/both), acceptance command
- Following the format defined in `ANVILML_DESIGN.md §17.1`

## Public API Surface

None. This task only adds test files — no new production `pub` items.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/tests/test_e2e_full_graph.py` | New Python integration test file with 3 tests exercising the full generic node graph via HTTP API |
| Modify | `docs/TESTS.md` | Add entries for the 3 new tests per `ANVILML_DESIGN.md §17.1` format |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `worker/tests/test_e2e_full_graph.py` | `test_full_graph_mock_mode` | Full Appendix B.2 ZiT graph executes end-to-end through generic node layer in mock mode, reaches Completed with a valid PNG artifact | Rust binary built; mock worker connects via ZeroMQ; all 8 node types registered | POST /v1/jobs with the full graph JSON | 202 Accepted → job transitions to Completed → GET /v1/artifacts/{hash} returns valid PNG | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_e2e_full_graph.py::test_full_graph_mock_mode -v --timeout=120` exits 0 |
| `worker/tests/test_e2e_full_graph.py` | `test_full_graph_real_mode` | Same graph in real mode with fixture checkpoints; artifact has correct dimensions (64×64) | Torch installed; fixture checkpoints registered in model store | POST /v1/jobs with the full graph JSON | 202 → Completed → valid 64×64 PNG | `worker/.venv/bin/python -m pytest worker/tests/test_e2e_full_graph.py::test_full_graph_real_mode -v --timeout=300` exits 0 |
| `worker/tests/test_e2e_full_graph.py` | `test_full_graph_invalid_graph_returns_400` | Structurally invalid graph (unknown node type) returns 400 Bad Request | Rust binary built | POST /v1/jobs with invalid graph JSON | 400 with validation error message | `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_e2e_full_graph.py::test_full_graph_invalid_graph_returns_400 -v --timeout=30` exits 0 |

## CI Impact

The new test file is picked up by the existing CI jobs:
- `worker-linux-mock`: runs `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests -v -m "not real_mode"` — picks up `test_full_graph_mock_mode` and `test_full_graph_invalid_graph_returns_400`
- `worker-linux-real`: runs `python -m pytest worker/tests -v -m real_mode` — picks up `test_full_graph_real_mode`
- `worker-windows-mock`/`worker-windows-real`: same on Windows paths

No CI workflow changes needed — the test follows the project's existing test marker convention (`real_mode` / non-marker for mock-compatible).

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. The test uses platform-agnostic subprocess spawning and HTTP calls. ZeroMQ TCP loopback is identical on Linux and Windows.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The Rust binary may not start quickly enough for the test to connect before timeout | Low | Medium | Use a health-check loop polling `GET /health` with 200ms intervals, up to 10s. The test timeout (60s for mock, 300s for real) provides ample headroom. |
| Real-mode test may exceed timeout if torch CPU inference is slow | Medium | High | Set a generous 300s timeout for the real-mode test. The Appendix B.2 graph uses steps=20 which is the design doc's default; the ACT agent can reduce to steps=4 if CI timing is an issue. |
| Model registration timing — fixture models must be registered before job submission | Medium | High | Include a `POST /v1/models/rescan` call after server startup to ensure models are registered. Poll the model list endpoint to confirm. |
| Mock worker's `Completed` event must include a valid base64-encoded PNG for artifact retrieval | Low | Medium | Pre-construct the 64×64 black PNG bytes in the test file, base64-encode them, and use as the mock image payload. This is the same pattern used by `SaveImage` mock tests. |

## Acceptance Criteria

- [ ] `ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/test_e2e_full_graph.py -v -m "not real_mode"` exits 0 (mock-mode tests pass)
- [ ] `worker/.venv/bin/python -m pytest worker/tests/test_e2e_full_graph.py -v -m real_mode` exits 0 (real-mode test passes)
- [ ] `grep -c "def test_" worker/tests/test_e2e_full_graph.py` returns >= 3 (at least 3 tests exist)
- [ ] `grep "REAL_PATH_VERIFIED:" worker/tests/test_e2e_full_graph.py || echo "N/A — no node execute() functions"` confirms no parity markers needed (this task adds tests, not node functions)
- [ ] `cargo test --workspace --features mock-hardware` exits 0 (no Rust test regressions)
