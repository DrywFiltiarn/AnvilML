# Tasks: Phase 012 — Job Submission & Queue

| Field | Value |
|-------|-------|
| Phase | 012 |
| Name | Job Submission & Queue |
| Milestone group | End-to-end generation (mock) |
| Depends on phases | 1-11 |
| Task file | `forge/tasks/tasks_phase012.json` |
| Tasks | 5 |

## Overview

Phase 12 persists and queues jobs: job DB row helpers, the in-memory `JobQueue`, `JobScheduler::submit` (validate -> persist as Queued -> enqueue -> notify -> broadcast `job.queued`), and the `GET /v1/jobs` list + `GET /v1/jobs/:id` endpoints. After this phase a submitted job is durably recorded as Queued and visible over the API (it does not yet run — dispatch is phase 13).

Every task in this phase implements **one module or one endpoint** plus its test. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the **Runnable Proof** below passes.

## Tasks

| Task | Module / File | Summary |
|------|-------------|---------|
| P12-A1 | `crates/anvilml-scheduler/src/job_store.rs` | anvilml-scheduler: job DB row helpers (insert, get, list, update status) |
| P12-A2 | `crates/anvilml-scheduler/src/queue.rs` | anvilml-scheduler: in-memory JobQueue |
| P12-A3 | `crates/anvilml-scheduler/src/scheduler.rs` | anvilml-scheduler: JobScheduler::submit (validate, persist, enqueue, notify) |
| P12-A4 | `crates/anvilml-server/src/handlers/jobs.rs` | anvilml-server: wire POST /v1/jobs to scheduler.submit + GET /v1/jobs/:id |
| P12-A5 | `crates/anvilml-server/src/handlers/jobs.rs` | anvilml-server: GET /v1/jobs list with status/limit/before |

## Task details

#### P12-A1: anvilml-scheduler: job DB row helpers (insert, get, list, update status)

- **Prereqs:** P11-A5
- **Tags:** —

Add sqlx + uuid + chrono to anvilml-scheduler. Create src/job_store.rs: async fn insert_job(pool,&Job), get_job(pool,id)->Option<Job>, list_jobs(pool, status:Option<JobStatus>, limit:u32, before:Option<DateTime<Utc>>)->Vec<Job>, update_status(pool,id,status,error:Option<&str>). Map graph/settings as TEXT JSON. cargo test -p anvilml-scheduler -- job_store exits 0 (tempfile DB: insert, get, list filter, status update).

#### P12-A2: anvilml-scheduler: in-memory JobQueue

- **Prereqs:** P12-A1
- **Tags:** —

Create src/queue.rs: JobQueue wrapping Mutex<VecDeque<Job>>. enqueue(job). cancel_queued(id)->bool (mark status Cancelled in place). pop_next()->Option<Job> (remove+return first Queued, skipping/removing Cancelled). len(). cargo test -p anvilml-scheduler -- queue exits 0: enqueue+pop order; cancel makes pop skip it.

#### P12-A3: anvilml-scheduler: JobScheduler::submit (validate, persist, enqueue, notify)

- **Prereqs:** P12-A2
- **Tags:** reasoning

Create src/scheduler.rs: JobScheduler{queue,workers,db,broadcaster,notify:Arc<Notify>}. async fn submit(&self,req)->Result<SubmitJobResponse,AnvilError>: validate_graph (Err->InvalidGraph), build Job{id,status:Queued,created_at,...}, insert_job, enqueue, broadcaster.send(JobQueued), notify.notify_one(), return {job_id, queue_position:len}. cargo test -p anvilml-scheduler --features mock-hardware -- submit exits 0: valid job persisted as Queued + JobQueued broadcast.

#### P12-A4: anvilml-server: wire POST /v1/jobs to scheduler.submit + GET /v1/jobs/:id

- **Prereqs:** P12-A3
- **Tags:** —

Add scheduler: Arc<JobScheduler> to AppState; construct in main.rs. Replace handlers/jobs.rs submit_job to call scheduler.submit (202 SubmitJobResponse, 422 on InvalidGraph). Add get_job(State,Path<Uuid>)->200 Job or 404 not_found. Wire GET /v1/jobs/:id. Verify: POST a valid ZiT graph -> 202 with job_id; curl /v1/jobs/<id> shows status Queued (it will not advance until phase 13 dispatch).

#### P12-A5: anvilml-server: GET /v1/jobs list with status/limit/before

- **Prereqs:** P12-A4
- **Tags:** —

Add list_jobs(State, Query{status:Option<JobStatus>, limit:Option<u32>, before:Option<DateTime<Utc>>})->Json<Vec<Job>> calling job_store::list_jobs (default limit 100, max 1000 clamp). Wire GET /v1/jobs. Verify: submit 2 jobs; curl '/v1/jobs' lists both; curl '/v1/jobs?status=queued' filters; ?limit=1 returns one.


## Runnable Proof

Submit a valid job and confirm it is persisted as Queued and listable.

```bash
cargo run --features mock-hardware
JOB=$(curl -s -X POST http://127.0.0.1:8488/v1/jobs \
  -H 'content-type: application/json' \
  -d @valid_zit_job.json | python -c 'import sys,json;print(json.load(sys.stdin)["job_id"])')
curl -s http://127.0.0.1:8488/v1/jobs/$JOB | python -m json.tool
curl -s 'http://127.0.0.1:8488/v1/jobs?status=queued' | python -m json.tool
```

Expected: submit returns 202 with a `job_id`; `GET /v1/jobs/:id` shows `status:"Queued"`; the list endpoint includes it and `?status=queued` filters to it. (A `valid_zit_job.json` body is the ZiT graph from phase 11.) Phase done when submitted jobs persist as Queued and are retrievable/listable.
