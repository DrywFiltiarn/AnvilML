# Tasks: Phase 016 — Job Cancellation

| Field | Value |
|-------|-------|
| Phase | 016 |
| Name | Job Cancellation |
| Milestone group | End-to-end generation (mock) |
| Depends on phases | 1-15 |
| Task file | `forge/tasks/tasks_phase016.json` |
| Tasks | 4 |

## Overview

Phase 16 adds cancellation: cooperative cancel in the worker (checks a flag between nodes), `JobScheduler::cancel` for both Queued and Running jobs, and `POST /v1/jobs/:id/cancel`. A per-node mock delay makes running-cancellation observable. After this phase you can cancel a job mid-flight and watch it settle to `Cancelled`.

Every task in this phase implements **one module or one endpoint** plus its test. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the **Runnable Proof** below passes.

## Tasks

| Task | Module / File | Summary |
|------|---------------|---------|
| P16-A1 | worker | worker: cooperative cancel — check cancel_flag between nodes |
| P16-A2 | anvilml-scheduler | anvilml-scheduler: JobScheduler::cancel (queued + running) |
| P16-A3 | `POST /v1/jobs/:id/cancel` | anvilml-server: POST /v1/jobs/:id/cancel |
| P16-A4 | `backend/tests/api_cancel.rs` | anvilml: integration test for cancel of a running mock job |

## Task details

#### P16-A1: worker: cooperative cancel — check cancel_flag between nodes

- **Prereqs:** P15-A3
- **Tags:** reasoning

In worker_main.py: maintain a per-job cancel flag set on receiving CancelJob{job_id}. In the mock Execute loop, before each node check the flag; if set emit Cancelled{job_id} and stop (no Completed). Add ANVILML_MOCK_NODE_DELAY_MS env to insert a sleep per node so cancellation is observable in tests. Verify via P16-A4.

#### P16-A2: anvilml-scheduler: JobScheduler::cancel (queued + running)

- **Prereqs:** P16-A1
- **Tags:** reasoning

Add to scheduler.rs: async fn cancel(&self,id)->Result<(),AnvilError>. Read job from DB: terminal -> Err(job_not_cancellable); Queued -> queue.cancel_queued+update_status Cancelled+broadcast JobCancelled; Running -> workers.send(owner, CancelJob{job_id})+update_status Cancelled+broadcast JobCancelled. Handle worker Cancelled event: set_idle (status already Cancelled), notify. Re-read status before applying late Completed. cargo test -p anvilml-scheduler --features mock-hardware -- cancel exits 0: queued cancel + running cancel both reach Cancelled.

#### P16-A3: anvilml-server: POST /v1/jobs/:id/cancel

- **Prereqs:** P16-A2
- **Tags:** —

Add handlers/jobs.rs cancel_job(State,Path<Uuid>): call scheduler.cancel; map JobNotFound->404 not_found, job_not_cancellable->409, success->202. Wire POST /v1/jobs/:id/cancel. Verify: submit a job with ANVILML_MOCK_NODE_DELAY_MS=300 so it stays Running; curl -X POST /v1/jobs/<id>/cancel -> 202; curl /v1/jobs/<id> -> Cancelled; cancel a Completed job -> 409.

#### P16-A4: anvilml: integration test for cancel of a running mock job

- **Prereqs:** P16-A3
- **Tags:** reasoning

Create backend/tests/api_cancel.rs: with mock worker + ANVILML_MOCK_NODE_DELAY_MS set, submit job, wait until Running, POST cancel, assert 202 + WS job.cancelled + GET job status Cancelled + worker returns to Idle within 3s. Also assert cancelling a terminal job returns 409. cargo test --features mock-hardware --test api_cancel exits 0.


## Runnable Proof

Submit a slow mock job and cancel it while it runs.

```bash
ANVILML_WORKER_MOCK=1 ANVILML_MOCK_NODE_DELAY_MS=400 ANVILML_VENV_PATH=./venv \
  cargo run --features mock-hardware
JOB=$(curl -s -X POST .../v1/jobs -d @valid_zit_job.json -H 'content-type: application/json' | jq -r .job_id)
sleep 0.5   # job is now Running
curl -s -X POST http://127.0.0.1:8488/v1/jobs/$JOB/cancel -i | head -1   # 202
curl -s http://127.0.0.1:8488/v1/jobs/$JOB | jq -r .status              # Cancelled
```

Expected: the cancel returns 202; the job status becomes `Cancelled`; the `/v1/events` stream shows `job.cancelled`; the worker returns to Idle. Cancelling a Completed job returns 409. Phase done when a running job can be cancelled and `cargo test --features mock-hardware --test api_cancel` is green.
