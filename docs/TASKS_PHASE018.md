# Tasks: Phase 018 — Worker Restart API & Preflight

| Field | Value |
|-------|-------|
| Phase | 018 |
| Name | Worker Restart API & Preflight |
| Milestone group | Production surface |
| Depends on phases | 1-17 |
| Task file | `forge/tasks/tasks_phase018.json` |
| Tasks | 4 |

## Overview

Phase 18 adds operational control: `WorkerPool::restart`/`shutdown_all`, `POST /v1/workers/:id/restart`, the real Python preflight that fills `EnvReport` (and makes `POST /v1/jobs` return 503 when the environment is unhealthy), and wires graceful shutdown to drain workers. After this phase you can repair a venv and restart a worker without stopping the server, and the env endpoint tells the truth.

Every task in this phase implements **one module or one endpoint** plus its test. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the **Runnable Proof** below passes.

## Tasks

| Task | Module / File | Summary |
|------|---------------|---------|
| P18-A1 | anvilml-worker | anvilml-worker: WorkerPool.restart + shutdown_all |
| P18-A2 | `POST /v1/workers/:id/restart` | anvilml-server: POST /v1/workers/:id/restart |
| P18-A3 | `backend/src/preflight.rs` | anvilml: Python preflight check populating EnvReport |
| P18-A4 | `backend/src/shutdown.rs` | anvilml: wire graceful shutdown to WorkerPool.shutdown_all |

## Task details

#### P18-A1: anvilml-worker: WorkerPool.restart + shutdown_all

- **Prereqs:** P17-A3
- **Tags:** reasoning

Add to pool.rs: async fn restart(&self,worker_id)->Result: send Shutdown, wait up to 5s for Dying, force-kill, re-spawn, re-send InitializeHardware. async fn shutdown_all(&self): Shutdown to each, wait up to 10s for Dying, force-kill stragglers. cargo test -p anvilml-worker --features mock-hardware -- restart exits 0: restart a mock worker -> it returns to Idle; shutdown_all stops all.

#### P18-A2: anvilml-server: POST /v1/workers/:id/restart

- **Prereqs:** P18-A1
- **Tags:** —

Add handlers/workers.rs restart_worker(State,Path<String>): 404 if worker_id unknown, else spawn workers.restart(id) and return 202. Wire POST /v1/workers/:id/restart. Verify: curl -X POST /v1/workers/worker-0/restart -> 202; /v1/workers shows it cycle Respawning then Idle; /v1/events shows the status transitions.

#### P18-A3: anvilml: Python preflight check populating EnvReport

- **Prereqs:** P18-A2
- **Tags:** reasoning

Create backend/src/preflight.rs: fn run_preflight(cfg)->EnvReport. Resolve interpreter (cross-platform); if missing preflight_ok=false reason=python_missing. Else run python --version (warn if not 3.12). If ANVILML_WORKER_MOCK unset run python -c 'import torch;print(torch.__version__)'; on failure preflight_ok=false reason=torch_unavailable. Store into AppState.env_report at startup (replace the stub from P3-A6). If preflight fails, POST /v1/jobs returns 503 workers_unavailable. Verify: curl /v1/system/env shows real python_path/version; with a broken venv, job submit -> 503.

#### P18-A4: anvilml: wire graceful shutdown to WorkerPool.shutdown_all

- **Prereqs:** P18-A3
- **Tags:** reasoning

Extend backend/src/shutdown.rs path: on shutdown signal, set a submissions-closed flag (POST /v1/jobs -> 503), call workers.shutdown_all(), close the sqlx pool (WAL flush), then exit 0. Verify: cargo run --features mock-hardware with a live worker; Ctrl-C; logs show workers receiving Shutdown and exiting, pool closed, process exits 0 within ~10s.


## Runnable Proof

Check real preflight, restart a worker, and confirm clean drain on shutdown.

```bash
ANVILML_VENV_PATH=./venv cargo run --features mock-hardware
curl -s http://127.0.0.1:8488/v1/system/env | jq    # real python_path/version, preflight_ok
curl -s -X POST http://127.0.0.1:8488/v1/workers/worker-0/restart -i | head -1   # 202
# watch /v1/workers cycle Respawning -> Idle; then Ctrl-C the server
```

Expected: `/v1/system/env` now shows the real interpreter path and version (and `preflight_ok:true` if torch is present, or a 503 on job submit if not); the restart returns 202 and the worker cycles back to Idle; Ctrl-C drains workers (Shutdown -> Dying) and exits 0. Phase done when restart works via REST and preflight reflects reality.
