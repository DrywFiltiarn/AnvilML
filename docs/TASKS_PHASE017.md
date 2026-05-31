# Tasks: Phase 017 — Job & Artifact Management

| Field | Value |
|-------|-------|
| Phase | 017 |
| Name | Job & Artifact Management |
| Milestone group | End-to-end generation (mock) |
| Depends on phases | 1-16 |
| Task file | `forge/tasks/tasks_phase017.json` |
| Tasks | 3 |

## Overview

Phase 17 adds deletion: `DELETE /v1/jobs/:id` (terminal jobs only, removes artifacts too) and `DELETE /v1/jobs?status=...` bulk clear. After this phase you can clean up completed jobs and reclaim their artifact files through the API.

Every task in this phase implements **one module or one endpoint** plus its test. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the **Runnable Proof** below passes.

## Tasks

| Task | Module / File | Summary |
|------|---------------|---------|
| P17-A1 | `DELETE /v1/jobs/:id` | anvilml-server: DELETE /v1/jobs/:id (terminal only, with artifacts) |
| P17-A2 | `DELETE /v1/jobs` | anvilml-server: DELETE /v1/jobs bulk clear by status |
| P17-A3 | `backend/tests/api_delete.rs` | anvilml: integration test for job + artifact deletion |

## Task details

#### P17-A1: anvilml-server: DELETE /v1/jobs/:id (terminal only, with artifacts)

- **Prereqs:** P16-A4
- **Tags:** —

Add to artifact/store.rs delete_for_job(job_id)->Result<u32> (delete on-disk files then DB rows for that job_id). Add handlers/jobs.rs delete_job(State,Path<Uuid>): read job; Running/Queued -> 409 job_active; else artifact_store.delete_for_job + DELETE jobs row -> 204. Wire DELETE /v1/jobs/:id. Verify: complete a job, DELETE it -> 204, GET it -> 404, its artifact file removed; deleting a running job -> 409.

#### P17-A2: anvilml-server: DELETE /v1/jobs bulk clear by status

- **Prereqs:** P17-A1
- **Tags:** reasoning

Add job_store helper delete_by_status(pool, status_filter)->Vec<Uuid> for completed|failed|cancelled|all (all = only terminal jobs). Add handlers/jobs.rs clear_jobs(State,Query{status})->Json{removed:u32}: for each matched terminal job, delete artifacts + row. Never delete Running/Queued. Wire DELETE /v1/jobs. Verify: create several terminal jobs; curl -X DELETE '/v1/jobs?status=completed' returns {removed:N}; list confirms they are gone; running jobs untouched.

#### P17-A3: anvilml: integration test for job + artifact deletion

- **Prereqs:** P17-A2
- **Tags:** —

Create backend/tests/api_delete.rs: run a mock job to Completed (with artifact), assert artifact file exists; DELETE /v1/jobs/:id -> 204; assert DB row gone + artifact file gone; assert DELETE on a Running job -> 409; bulk DELETE ?status=all removes all terminal jobs. cargo test --features mock-hardware --test api_delete exits 0.


## Runnable Proof

Complete a job, delete it, and confirm both the record and the file are gone.

```bash
# run a job to Completed (as in phase 14), capture $JOB and $HASH
curl -s -X DELETE http://127.0.0.1:8488/v1/jobs/$JOB -i | head -1     # 204
curl -s http://127.0.0.1:8488/v1/jobs/$JOB -i | head -1               # 404
ls artifacts/${HASH:0:2}/$HASH.png 2>&1                               # No such file
# bulk:
curl -s -X DELETE 'http://127.0.0.1:8488/v1/jobs?status=all'         # {"removed":N}
```

Expected: single delete returns 204, the job then 404s, and its PNG is removed from disk; deleting a Running job returns 409; bulk clear returns `{removed:N}`. Phase done when terminal jobs + artifacts delete cleanly and `cargo test --features mock-hardware --test api_delete` is green.
