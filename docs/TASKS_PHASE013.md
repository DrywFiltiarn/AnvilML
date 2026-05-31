# Tasks: Phase 013 — Dispatch & Execute

| Field | Value |
|-------|-------|
| Phase | 013 |
| Name | Dispatch & Execute |
| Milestone group | End-to-end generation (mock) |
| Depends on phases | 1-12 |
| Task file | `forge/tasks/tasks_phase013.json` |
| Tasks | 6 |

## Overview

Phase 13 closes the loop: the `VramLedger`, the `select_worker` algorithm, the background dispatch loop that assigns Queued jobs to Idle workers via `Execute`, a minimal mock executor that emits Progress + Completed, and the scheduler handling of Completed/Failed. After this phase a submitted job actually runs on the mock worker and reaches `Completed`.

Every task in this phase implements **one module or one endpoint** plus its test. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the **Runnable Proof** below passes.

## Tasks

| Task | Module / File | Summary |
|------|-------------|---------|
| P13-A1 | `crates/anvilml-scheduler/src/ledger.rs` | anvilml-scheduler: VramLedger |
| P13-A2 | `crates/anvilml-scheduler/src/scheduler.rs` | anvilml-scheduler: select_worker (preference/auto/cpu) |
| P13-A3 | `crates/anvilml-scheduler/src/scheduler.rs` | anvilml-scheduler: dispatch loop (Queued -> Execute on idle worker) |
| P13-A4 | `worker/worker_main.py` | worker: mock executor returning Completed (no image yet) |
| P13-A5 | `crates/anvilml-scheduler/src/scheduler.rs` | anvilml-scheduler: handle worker Completed/Failed -> terminal status + idle |
| P13-A6 | `backend/src/main.rs` | anvilml: start dispatch loop at startup; verify job reaches Completed |

## Task details

#### P13-A1: anvilml-scheduler: VramLedger

- **Prereqs:** P12-A5
- **Tags:** —

Create src/ledger.rs: VramLedger{devices:HashMap<u32,(u32 total,u32 used)>}. update(idx,used,total); free_mib(idx)->u32 (0 if unknown); would_fit(idx,req)->bool. init_from(hw:&HardwareInfo). cargo test -p anvilml-scheduler -- ledger exits 0: init, update, would_fit, unknown device returns 0.

#### P13-A2: anvilml-scheduler: select_worker (preference/auto/cpu)

- **Prereqs:** P13-A1
- **Tags:** reasoning

Add to scheduler.rs: fn select_worker(job:&Job, workers:&[WorkerInfo], ledger:&VramLedger, default_device:&str)->Option<usize>. device_preference Some(n): that worker if Idle else None. auto: Idle workers ranked by free_mib desc, tie device_index asc, pick top. 'cpu': only the Cpu worker. cargo test -p anvilml-scheduler -- select exits 0: all three modes + busy-preferred returns None.

#### P13-A3: anvilml-scheduler: dispatch loop (Queued -> Execute on idle worker)

- **Prereqs:** P13-A2
- **Tags:** reasoning

Add to scheduler.rs: start_dispatch_loop()->JoinHandle. Wakes on notify OR worker event (subscribe_events). Per wake loop: pop_next Queued job, select_worker; on match update_status Running(started_at,worker_id,device_index), workers.set_busy, broadcaster.send(JobStarted), workers.send(Execute{...}); repeat until no match. tokio::sync::Mutex held across await. cargo test -p anvilml-scheduler --features mock-hardware -- dispatch exits 0: submitted job causes Execute sent to mock worker.

#### P13-A4: worker: mock executor returning Completed (no image yet)

- **Prereqs:** P13-A3
- **Tags:** —

In worker_main.py handle Execute in mock mode: parse graph nodes, for each emit Progress{node_index,node_total,node_type}, then emit Completed{job_id, elapsed_ms}. No image/artifact yet (phase 14). Keep it minimal: iterate graph['nodes'] in given order. Verify proof deferred to P13-A6.

#### P13-A5: anvilml-scheduler: handle worker Completed/Failed -> terminal status + idle

- **Prereqs:** P13-A4
- **Tags:** reasoning

In dispatch loop event handling: on WorkerEvent::Completed{job_id} update_status Completed(completed_at), workers.set_idle, broadcaster.send(JobCompleted), notify.notify_one(). On Failed update_status Failed(error), set_idle, broadcast JobFailed, notify. Re-read job status from DB before applying (ignore events for already-terminal jobs). cargo test -p anvilml-scheduler --features mock-hardware -- complete exits 0: mock job reaches Completed in DB.

#### P13-A6: anvilml: start dispatch loop at startup; verify job reaches Completed

- **Prereqs:** P13-A5
- **Tags:** —

In main.rs call scheduler.start_dispatch_loop() at startup. Ensure scheduler holds Arc<WorkerPool> + broadcaster + db. Verify end-to-end: ANVILML_WORKER_MOCK=1 ANVILML_VENV_PATH=<venv> cargo run --features mock-hardware; POST a valid ZiT graph to /v1/jobs; poll curl /v1/jobs/<id> and observe status transition Queued -> Running -> Completed within a few seconds.


## Runnable Proof

Submit a job and watch it transition all the way to Completed.

```bash
ANVILML_WORKER_MOCK=1 ANVILML_VENV_PATH=./venv \
  cargo run --features mock-hardware
JOB=$(curl -s -X POST http://127.0.0.1:8488/v1/jobs -H 'content-type: application/json' -d @valid_zit_job.json | python -c 'import sys,json;print(json.load(sys.stdin)["job_id"])')
# poll:
for i in $(seq 1 10); do curl -s http://127.0.0.1:8488/v1/jobs/$JOB | python -c 'import sys,json;print(json.load(sys.stdin)["status"])'; sleep 1; done
```

Expected: the polled status prints `Queued` then `Running` then `Completed` within a few seconds. Phase done when a submitted mock job reaches `Completed` in the DB and `cargo test -p anvilml-scheduler --features mock-hardware` is green.
