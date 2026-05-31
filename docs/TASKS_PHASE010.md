# Tasks: Phase 010 — Worker Crash Recovery

| Field | Value |
|-------|-------|
| Phase | 010 |
| Name | Worker Crash Recovery |
| Milestone group | Worker lifecycle |
| Depends on phases | 1-9 |
| Task file | `forge/tasks/tasks_phase010.json` |
| Tasks | 4 |

## Overview

Phase 10 implements the watchdog: 30s keepalive ping with force-kill on Pong timeout, automatic respawn (2s delay) after a worker dies, orphan cleanup (PR_SET_PDEATHSIG / Job Object), and bridging worker status changes onto the WebSocket. After this phase you can kill a worker process and watch the server detect it, mark it Dead, and respawn it to Idle without restarting the server.

Every task in this phase implements **one module or one endpoint** plus its test. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the **Runnable Proof** below passes.

## Tasks

| Task | Module / File | Summary |
|------|-------------|---------|
| P10-A1 | `crates/anvilml-worker/src/pool.rs` | anvilml-worker: keepalive Ping + Pong-timeout force-kill |
| P10-A2 | `crates/anvilml-worker/src/managed.rs` | anvilml-worker: respawn after death (2s delay) + WorkerStatusChanged events |
| P10-A3 | `backend/src/main.rs` | anvilml-server: broadcast worker status changes to WS |
| P10-A4 | `crates/anvilml-worker/src/pool.rs` | anvilml: test-only worker PID accessor for crash-recovery proof |

## Task details

#### P10-A1: anvilml-worker: keepalive Ping + Pong-timeout force-kill

- **Prereqs:** P9-A6
- **Tags:** reasoning

In pool.rs / managed.rs add a keepalive task per worker: send Ping{seq} every 30s, expect Pong{seq} within 10s; on timeout call Child::kill() (SIGKILL unix / TerminateProcess windows via tokio Child::kill). For testability allow the interval to be overridden via ANVILML_PING_INTERVAL_MS / ANVILML_PONG_TIMEOUT_MS env (used by tests). cargo test -p anvilml-worker --features mock-hardware -- keepalive exits 0: with short intervals, a mock worker that stops responding is killed. Also pass: cargo check --target x86_64-pc-windows-gnu --features mock-hardware.

#### P10-A2: anvilml-worker: respawn after death (2s delay) + WorkerStatusChanged events

- **Prereqs:** P10-A1
- **Tags:** reasoning

In managed.rs: on detecting Dead (EOF or ping timeout), broadcast WorkerStatusChanged(Dead), wait 2s (override via ANVILML_RESPAWN_DELAY_MS), transition Respawning (broadcast), re-spawn the child, re-send InitializeHardware, on Ready broadcast Idle. Orphan cleanup at spawn: Linux PR_SET_PDEATHSIG via pre_exec; Windows Job Object KILL_ON_JOB_CLOSE. cargo test -p anvilml-worker --features mock-hardware -- respawn exits 0: kill worker, observe Dead->Respawning->Idle within timeout. Also pass: cargo check --target x86_64-pc-windows-gnu --features mock-hardware.

#### P10-A3: anvilml-server: broadcast worker status changes to WS

- **Prereqs:** P10-A2
- **Tags:** —

In main.rs / server wiring: subscribe to WorkerPool.subscribe_events(); for each WorkerStatusChanged forward as WsEvent::WorkerStatusChanged via the EventBroadcaster so /v1/events clients see worker state transitions. Spawn this bridge task at startup. cargo test --workspace --features mock-hardware exits 0. Verified live in P10-A4.

#### P10-A4: anvilml: test-only worker PID accessor for crash-recovery proof

- **Prereqs:** P10-A3
- **Tags:** —

Add #[cfg(any(test,feature="test-helpers"))] fn pid_for(&self,worker_id:&str)->Option<u32> on WorkerPool returning the child PID. This enables the runnable crash-recovery proof and the integration test in later phases. cargo test --workspace --features mock-hardware exits 0. Runnable proof: ANVILML_WORKER_MOCK=1 cargo run --features mock-hardware; note worker PID from logs; kill <pid>; watch /v1/workers (or /v1/events) show Dead then Respawning then Idle within ~3s; server stays up. Also pass: cargo check --target x86_64-pc-windows-gnu --features mock-hardware.


## Runnable Proof

Start the server with a worker, kill the worker process, and watch it recover.

```bash
ANVILML_WORKER_MOCK=1 ANVILML_VENV_PATH=./venv \
  cargo run --features mock-hardware
# note the worker PID from the startup logs, then:
kill -9 <worker_pid>
# watch the worker recover:
watch -n0.5 'curl -s http://127.0.0.1:8488/v1/workers'
```

Expected: `/v1/workers` (or the `/v1/events` stream) shows the worker go `dead` -> `respawning` -> `idle` within ~3 seconds, and the server process itself stays up the whole time. Phase done when a killed worker is automatically respawned to Idle and the server never goes down.
