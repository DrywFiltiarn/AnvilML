# Plan Report: P18-A3

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P18-A3                                            |
| Phase       | 018 — Worker Restart API & Preflight              |
| Description | anvilml: Python preflight check populating EnvReport |
| Depends on  | P18-A2                                            |
| Project     | anvilml                                           |
| Planned at  | 2026-06-10T19:10:27Z                              |
| Attempt     | 1                                                 |

## Objective

Implement the real Python preflight check that resolves the interpreter path, runs `python --version` and (when not in mock mode) `import torch`, populates the `EnvReport` struct in `AppState`, and gates job submission with a 503 when the environment is unhealthy.

## Scope

### In Scope
- Create `backend/src/preflight.rs` with `run_preflight(cfg: &ServerConfig) -> EnvReport`
- Resolve interpreter path cross-platform: `{venv_path}/bin/python3` (Unix) or `{venv_path}\Scripts\python.exe` (Windows)
- Check interpreter exists and is executable
- Run `python --version` and parse version string; log WARN if not 3.12.x
- When `ANVILML_WORKER_MOCK` is unset, run `python -c 'import torch;print(torch.__version__)'` to verify torch availability
- Populate `EnvReport` with `python_path`, `python_version`, `torch_version`, `preflight_ok`, `reason`
- Wire preflight into `backend/src/main.rs` startup before `WorkerPool::spawn_all`
- Update `AppState.env_report` with preflight results at startup
- Gate `POST /v1/jobs` with 503 `workers_unavailable` when `preflight_ok == false`
- Update `handlers/system.rs` doc comment to reflect live data (remove "stubbed" language)
- Update integration test in `crates/anvilml-server/src/lib.rs` (`env_returns_200_with_stub_report`) to verify EnvReport fields are populated after preflight
- Add integration test verifying job submit returns 503 when preflight fails

### Out of Scope
- Worker restart API (P18-A1, P18-A2 — separate tasks)
- Graceful shutdown wiring (P18-A4 — separate task)
- Auto-provisioning of venv (Phase 23)
- Re-running preflight on worker restart (deferred to later phase if needed)

## Approach

1. **Create `backend/src/preflight.rs`** with three functions:
   - `resolve_interpreter(venv_path: &Path) -> PathBuf` — returns `{venv_path}/bin/python3` on Unix, `{venv_path}\Scripts\python.exe` on Windows
   - `run_preflight(cfg: &ServerConfig) -> EnvReport` — orchestrates the checks:
     a. Resolve interpreter path via `resolve_interpreter`
     b. Check `fs::metadata(path).is_ok()` — if missing, return `EnvReport { python_path, preflight_ok: false, reason: "python_missing", ..default() }`
     c. Spawn `cmd!(path, "--version")`, parse stdout for version (e.g. "Python 3.12.4")
     d. Log WARN if major.minor != "3.12"
     e. If `std::env::var("ANVILML_WORKER_MOCK").is_err()` (unset), spawn `cmd!(path, "-c", "import torch;print(torch.__version__)")` — parse output for torch version
     f. On torch failure, return `EnvReport { ..., preflight_ok: false, reason: "torch_unavailable" }`
     g. On success, return `EnvReport { python_path, python_version, torch_version, preflight_ok: true, reason: "" }`

2. **Wire preflight into `backend/src/main.rs`**:
   - After hardware detection and DB open, before `WorkerPool::spawn_all`, call `let env_report = preflight::run_preflight(&cfg);`
   - Log the result: `tracing::info!(preflight_ok = env_report.preflight_ok, python_path = %env_report.python_path, python_version = %env_report.python_version, torch_version = %env_report.torch_version, reason = %env_report.reason, "python preflight complete");`
   - Update `AppState.env_report` by writing to the `Arc<RwLock<EnvReport>>` field: `state.env_report.write().unwrap().clone_from(&env_report);`
   - Note: Since `App::new_with_hardware` already creates `env_report` with stub values, we need to write the preflight result into the existing `Arc<RwLock<EnvReport>>`. This requires adding a setter method to `AppState` or writing directly via the `RwLock`.

3. **Add `env_report` setter to `AppState`** (`crates/anvilml-server/src/state.rs`):
   - Add `pub fn set_env_report(&self, report: EnvReport)` that writes into the `Arc<RwLock<EnvReport>>`
   - This keeps the existing `env_report()` getter unchanged

4. **Gate job submission** (`crates/anvilml-server/src/handlers/jobs.rs`):
   - At the top of `submit_job`, before the scheduler check, read `state.env_report()` and check `preflight_ok`
   - If `preflight_ok == false`, return `StatusCode::SERVICE_UNAVAILABLE` with body `{"error": "workers_unavailable", "message": "python preflight failed: <reason>", "request_id": "<uuid>"}`
   - This is checked before any scheduler validation

5. **Update system handler doc comment** (`crates/anvilml-server/src/handlers/system.rs`):
   - Change "stubbed" language to reflect live data from preflight

6. **Update existing integration test** (`crates/anvilml-server/src/lib.rs`):
   - The `env_returns_200_with_stub_report` test asserts stub values — update it to verify the handler still returns 200 with correct shape, but acknowledge that in tests (no preflight runs), stub values remain. No change needed since tests use `App::new()` which sets stub values directly.

7. **Add new integration test** in `backend/tests/`:
   - `preflight_check.rs` — test that verifies:
     a. When preflight succeeds, `GET /v1/system/env` returns `preflight_ok: true` with populated paths
     b. When preflight fails, `POST /v1/jobs` returns 503 with `workers_unavailable`

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `backend/src/preflight.rs` | New module: `run_preflight(cfg) -> EnvReport`, `resolve_interpreter(venv_path)` |
| MODIFY | `backend/src/main.rs` | Wire preflight call before `WorkerPool::spawn_all`; update `AppState.env_report` with result |
| MODIFY | `crates/anvilml-server/src/state.rs` | Add `set_env_report(&self, EnvReport)` method to `AppState` |
| MODIFY | `crates/anvilml-server/src/handlers/jobs.rs` | Gate `submit_job` with preflight check → 503 `workers_unavailable` |
| MODIFY | `crates/anvilml-server/src/handlers/system.rs` | Update doc comment to remove "stubbed" language |
| MODIFY | `backend/Cargo.toml` | Add `regex` dependency (for version parsing) — check if already available in workspace |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `backend/tests/preflight_check.rs` (new) | `preflight_success_returns_env_report` | `GET /v1/system/env` returns `preflight_ok: true` with `python_path` populated when venv exists |
| `backend/tests/preflight_check.rs` (new) | `preflight_failure_returns_503_on_job_submit` | `POST /v1/jobs` returns 503 `workers_unavailable` when preflight fails |
| `crates/anvilml-server/src/lib.rs` (modify) | `env_returns_200_with_stub_report` | Existing test still passes (stub values in test context) |

## CI Impact

The `preflight_ok` check in `submit_job` adds a new code path. The existing test `env_returns_200_with_stub_report` in `anvilml-server/src/lib.rs` uses `App::new()` which initializes `env_report` with stub values (`preflight_ok: false`). Since the test does not call `submit_job`, no regression is expected. However, any existing `submit_job` tests that rely on `preflight_ok: false` returning 202 will need the preflight gate to be bypassed in test context (e.g., by setting `ANVILML_WORKER_MOCK=1` or by checking `preflight_ok` only when not in test mode). The plan addresses this: in test context, `preflight_ok` is `false` from the stub, so `submit_job` will return 503. The existing `submit_job_valid_zit_graph_returns_202` test in `jobs.rs` will need to be updated to either set `ANVILML_WORKER_MOCK=1` or update the `env_report` before submitting.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `submit_job` tests break because stub `preflight_ok: false` now returns 503 | High | Medium — existing tests in `jobs.rs` expect 202 | Set `ANVILML_WORKER_MOCK=1` in test setup, or add a `bypass_preflight` flag to `EnvReport`. The cleaner approach: in `submit_job`, also check `ANVILML_WORKER_MOCK` env var — if set, skip the preflight gate (mock mode means no real Python needed). |
| Cross-platform path resolution fails on Windows | Medium | High — `preflight.rs` is in backend crate | Use `std::path::Path` and `cfg!(windows)` for path separator; test with cross-compilation target. |
| `python --version` output format varies across Python distributions | Medium | Low — parse conservatively, accept any "Python X.Y.Z" format | Use regex to extract version; fall back to raw stdout on parse failure. |
| `tokio::process::Command` needs `tokio` feature — already in workspace | Low | None — `tokio` is already a backend dependency | Verify `tokio` has `process` feature enabled in workspace Cargo.toml. |
| Pre-existing test `env_returns_200_with_stub_report` still asserts stub values | Low | Low — test is unchanged, stub values remain in test context | No change needed; the test context never runs preflight. |

## Acceptance Criteria

- [ ] `backend/src/preflight.rs` exists with `run_preflight(cfg: &ServerConfig) -> EnvReport` and `resolve_interpreter` helper
- [ ] Preflight resolves `{venv_path}/bin/python3` on Unix and `{venv_path}\Scripts\python.exe` on Windows
- [ ] Missing interpreter produces `preflight_ok: false, reason: "python_missing"`
- [ ] Present interpreter runs `python --version`; version parsed and stored in `python_version`
- [ ] WARN logged when Python version is not 3.12.x
- [ ] When `ANVILML_WORKER_MOCK` unset, `import torch` is verified; torch version stored
- [ ] Torch failure produces `preflight_ok: false, reason: "torch_unavailable"`
- [ ] `AppState.env_report` is updated with preflight results at startup in `main.rs`
- [ ] `GET /v1/system/env` returns real `python_path` and `python_version` (verified via integration test)
- [ ] `POST /v1/jobs` returns 503 with `workers_unavailable` when `preflight_ok == false`
- [ ] When `ANVILML_WORKER_MOCK=1`, preflight gate is bypassed (job submit proceeds normally)
- [ ] `cargo test -p backend --features mock-hardware -- preflight` exits 0
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
