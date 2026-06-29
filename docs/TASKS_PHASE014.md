# Tasks: Phase 14 — Dispatch & Execute

**Phase:** 14
**Name:** Dispatch & Execute
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 2, 3, 5, 6, 8, 9, 10, 12, 13

---

## Overview

This phase completes `JobScheduler` (submission, cancellation, lookup, and the
notify-driven dispatch loop with its exact worker-selection algorithm), introduces
the very first concrete node in the project (`PassThrough`, deliberately trivial),
wires `backend/main.rs` to actually spawn a real `WorkerPool` from its normal
server-start path for the first time, and exposes job submission and querying over
HTTP via `POST /v1/jobs` and `GET /v1/jobs*`. This is the phase where a submitted job
genuinely executes end to end — real dispatch, a real (if trivial) node, not a mock
stand-in for either.

This phase exists at this point because every piece it needs already exists in
isolation from earlier phases — the queue and ledger (Phase 13), the validator
(Phase 12), the worker pool (Phase 8), real worker startup (Phase 9), and the node
base contract (Phase 10) — but nothing yet ties them into one running pipeline. The
original roadmap for this phase explicitly calls for "a trivial real node (e.g. a
no-op pass-through)" to prove genuine end-to-end real dispatch, "not just mock" —
this phase's `PassThrough` node is exactly that, and deliberately nothing more; the
real baseline node set (`LoadModel`, `Sampler`, etc.) remains out of scope for later,
separately-authored phases.

At the start of this phase, `JobScheduler` doesn't exist, `backend/main.rs` never
spawns a real `WorkerPool`, and `worker/nodes/` has no concrete node files. At the
end: a submitted job is validated, queued, persisted, dispatched to a real spawned
worker subprocess according to the documented worker-selection algorithm, executed by
a real (trivial) node, and observably reaches `Completed` — provable by polling
`GET /v1/jobs/:id` against a live server.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | JobScheduler | P14-A1 … P14-A5 | `submit()`, `cancel()`/`get_job()`, the dispatch loop skeleton, real worker selection, then marking the assigned worker `Busy` |
| B | First concrete node | P14-B1 | `PassThrough` — the project's first real node file |
| C | Server wiring | P14-C1 … P14-C2 | `AppState` gains scheduler/worker/db fields; `main.rs` spawns a real `WorkerPool` |
| D | HTTP handlers | P14-D1 … P14-D2 | `POST /v1/jobs`, then `GET /v1/jobs` and `GET /v1/jobs/:id` |
| E | Proof | P14-E1 | The phase's Runnable Proof — a job reaching `Completed` |

---

## Prerequisites

`anvilml-scheduler` must have `JobQueue`/`VramLedger` (Phase 13) and
`ValidatedGraph`/`GraphError`/`validate_graph()` (Phase 12). `WorkerPool` must exist
and pass its own tests per Phase 8. `worker/nodes/base.py`'s `BaseNode`/`@register`
contract must exist per Phase 10 (P10-A4). `JobStore`'s ghost-job reset must already
be wired into `backend/main.rs` per Phase 13 (P13-C1).

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §12.5` | P14-A3, P14-A4, P14-A5 | The exact 2-step worker selection algorithm, in order, plus the resulting `Busy` status transition |
| `ANVILML_DESIGN.md §12.2` | P14-A1 | An empty `NodeTypeRegistry` means every job submission returns `503 workers_unavailable` |
| `ANVILML_DESIGN.md §10.6` | P14-B1 | The `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` marker pair, exercised for the first time on a real node file |
| `ANVILML_DESIGN.md §13.2`, §3.3 | P14-C1, P14-D1 | `AppState`'s field growth pattern; "no business logic in handler functions" |
| `ANVILML_DESIGN.md §13.4`, §13.5 | P14-D1, P14-D2 | Exact route shapes and HTTP status codes, including `503`/`422`/`404` |

---

## Task Descriptions

### Group A — JobScheduler

#### P14-A1: anvilml-scheduler: JobScheduler struct + submit()

**Goal:** Implement the scheduler's struct shape and the job submission path —
validation, persistence, enqueueing, and waking the (not-yet-existing) dispatch
loop — all in one coherent piece.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/scheduler.rs` — `JobScheduler`, `submit()`.

**Key implementation notes:**
- `queue`/`ledger` are wrapped in `tokio::sync::Mutex`, **not** `std::sync::Mutex` —
  this is held across `await` points when persisting to `job_store`, a hard
  requirement, not a stylistic preference.
- An empty `node_registry` short-circuits `submit()` with
  `AnvilError::WorkersUnavailable` **before** validation even runs — per
  `ANVILML_DESIGN.md §12.2`, no worker has reached `Ready` yet means no job
  submission can succeed, regardless of how well-formed the graph is.

**Acceptance criterion:**
```bash
cargo test -p anvilml-scheduler --test scheduler_tests
# -> >=4 tests, exits 0
```

#### P14-A2: anvilml-scheduler: JobScheduler cancel()/get_job()

**Goal:** Complete the scheduler's read/cancel surface, including the
authoritative lookup path for jobs that have already left the in-memory queue.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/scheduler.rs` — adds `cancel()`, `get_job()`.

**Key implementation notes:**
- `get_job()` delegates to `job_store.get()`, **not** the in-memory queue — a
  `Completed` or `Failed` job has already left the queue, but must still be
  queryable from the database, which is authoritative for terminal-state jobs.

**Acceptance criterion:**
```bash
cargo test -p anvilml-scheduler --test scheduler_tests
# -> >=8 tests total in the file, exits 0
```

#### P14-A3: anvilml-scheduler: dispatch loop skeleton, notify-driven wake

**Goal:** Implement the dispatch loop's outer structure — the notify-driven wake
cycle and the per-job iteration — with a deliberately stubbed selection step, before
the real selection logic is added.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/scheduler.rs` — adds `start_dispatch_loop()`,
  `dispatch_one()` (stub).

**Key implementation notes:**
- This task's `dispatch_one()` always returns `false` — no actual worker selection
  yet, that's the next task's scope. The point of this task is proving the wake
  cycle itself works correctly in isolation from the selection algorithm.
- A worker-idle-triggered wake (in addition to the `submit()`-triggered one this
  task wires) is explicitly deferred to a later task — this task only wires the
  submission-triggered path.
- Must not block the async runtime — no synchronous I/O inside the loop body.

**Acceptance criterion:**
```bash
cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests
# -> >=11 tests total in the file, exits 0
```

#### P14-A4: anvilml-scheduler: worker selection algorithm, real dispatch

**Goal:** Replace the stub with the real, exact worker-selection algorithm from
the design document, completing the dispatch loop's job-side responsibilities.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/scheduler.rs` — replaces `dispatch_one()`'s stub.

**Key implementation notes:**
- The 2-step algorithm, in order, per `ANVILML_DESIGN.md §12.5`: (1) an `Idle`
  worker matching `job.settings.device_preference` wins outright if one exists;
  (2) otherwise rank `Idle` workers by `vram_free_mib` descending and pick the top.
  If no worker is `Idle`, the job stays queued — this is not an error condition.
- On a successful match: reserve VRAM via the ledger, transition the job to
  `Running`, persist, and send `WorkerMessage::Execute` — all four steps happen
  together, not as separable partial states.
- Marking the assigned **worker's own** status `Busy` is explicitly deferred to the
  next task — this task only selects and dispatches; it does not yet close the loop
  on the worker's own status reflecting that it's now occupied.

**Acceptance criterion:**
```bash
cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests
# -> >=17 tests total in the file, exits 0
```

#### P14-A5: anvilml-scheduler: dispatch_one marks the assigned worker Busy

**Goal:** Close the gap P14-A4 left open — without this task, a worker's own
`WorkerHandle` still reads `Idle` immediately after being assigned a job, which
means a second dispatch cycle could select the **same** worker again before it
finishes the first one.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/scheduler.rs` — extends `dispatch_one()`.

**Key implementation notes:**
- Calls `worker_handle.set_status(WorkerStatus::Busy)` (Phase 8's `P8-E2`)
  immediately after a successful selection, alongside the VRAM reservation.
- `Idle` restoration on job completion, and waking the dispatch loop when that
  happens, are both separate, later-phase concerns — this task closes only the
  assignment-time half of the worker status lifecycle.

**Acceptance criterion:**
```bash
cargo test -p anvilml-scheduler --features mock-hardware --test scheduler_tests
# -> >=21 tests total in the file, exits 0
```

---

### Group B — First concrete node

#### P14-B1: worker/nodes/passthrough.py: trivial real node (no-op)

**Goal:** Create the project's first concrete node file — deliberately trivial,
existing solely to prove the dispatch pipeline and the mock/real marker convention
both work end-to-end on a real file, not just in the abstract.

**Files to create or modify:**
- `worker/nodes/passthrough.py` — `PassThrough`.

**Key implementation notes:**
- A pass-through node has no meaningfully different mock-vs-real behavior — both
  branches of `execute()` simply return the input unchanged — but the
  `ctx.mock`-branching structure and the `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED`
  marker pair are still required, per `ANVILML_DESIGN.md §10.6`, with no exception
  carved out for trivial nodes.
- Uses `SlotType::Any` for both its input and output slot — appropriate for a node
  that genuinely doesn't care what type of data flows through it.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_passthrough.py -v
# -> >=5 tests, exits 0
```

---

### Group C — Server wiring

#### P14-C1: anvilml-server: AppState gains scheduler/workers/db fields

**Goal:** Grow `AppState` with exactly the three new fields this phase's handlers
need, continuing Phase 11's pattern of incremental, non-speculative field growth.

**Files to create or modify:**
- `crates/anvilml-server/src/state.rs` — adds `scheduler`, `workers`, `db`.

**Key implementation notes:**
- `node_registry` already exists from Phase 11 — do not duplicate it.
- The remaining `ANVILML_DESIGN.md §13.2` fields (`hardware`, `broadcaster`,
  `artifact_store`, `env_report`) stay absent — added only when a later task
  actually needs them.
- Wiring `backend/main.rs` to actually construct and spawn a real `WorkerPool` is
  explicitly deferred to the next task; this task is the struct shape alone.

**Acceptance criterion:**
```bash
cargo test -p anvilml-server --test state_tests
# -> >=5 tests total in the file, exits 0
```

#### P14-C2: backend: main.rs spawns real WorkerPool + JobScheduler at startup

**Goal:** Connect the binary's normal startup path to a real, spawned
`WorkerPool` and a running `JobScheduler` — the first time any worker subprocess is
spawned outside of a dedicated test.

**Files to create or modify:**
- `backend/src/main.rs` — constructs `WorkerPool`, calls `spawn_all()` against
  `detect_all_devices()`'s result, constructs `JobScheduler`, calls
  `start_dispatch_loop()`, builds the now-larger `AppState`.

**Key implementation notes:**
- This receives exactly the wiring scope P14-C1 deferred.
- This is tagged `breaking` because it changes what `main.rs`'s normal run path
  actually does at startup — confirm existing `/health`, `/v1/nodes`, and
  `hw-probe` behavior is unaffected.

**Acceptance criterion:**
```bash
cargo build -p anvilml
cargo test --workspace --features mock-hardware
# -> both exit 0
```

---

### Group D — HTTP handlers

#### P14-D1: anvilml-server: POST /v1/jobs handler

**Goal:** Expose job submission over HTTP for the first time, delegating entirely
to the scheduler with zero business logic in the handler itself.

**Files to create or modify:**
- `crates/anvilml-server/src/handlers/jobs.rs` — `submit_job()`.

**Key implementation notes:**
- `SubmitJobRequest`/`SubmitJobResponse` are new, small HTTP-layer structs defined
  in this file, not `anvilml-core` — they're a distinct concern from the domain
  `Job` type.
- The handler is a one-line delegation to `state.scheduler.submit()` — `AnvilError`'s
  existing `IntoResponse` impl (Phase 2) already handles the `503`/`422` mapping
  automatically, no special-casing needed in the handler.

**Acceptance criterion:**
```bash
cargo test -p anvilml-server --test jobs_tests
# -> >=4 tests, exits 0
```

#### P14-D2: anvilml-server: GET /v1/jobs and GET /v1/jobs/:id handlers

**Goal:** Complete the jobs handler set with the two read endpoints, closing out
this phase's HTTP surface.

**Files to create or modify:**
- `crates/anvilml-server/src/handlers/jobs.rs` — adds `list_jobs()`, `get_job()`.

**Key implementation notes:**
- `ListJobsParams.before` is kept in the query struct for forward-compatibility
  with `ANVILML_DESIGN.md §13.4`'s documented `?before=` parameter, even if
  `job_store.list()` doesn't yet support a before-cursor — if so, record this as a
  Deviation rather than silently dropping the field from the struct.
- `get_job()` on an unknown ID returns `404` via `AnvilError::JobNotFound`.

**Acceptance criterion:**
```bash
cargo test -p anvilml-server --test jobs_tests
# -> >=9 tests total in the file, exits 0
```

---

### Group E — Proof

#### P14-E1: Runnable Proof: submitted job with PassThrough node reaches Completed

**Goal:** Produce this phase's Runnable Proof — the first genuine end-to-end
demonstration of real dispatch against a real node — and record the transcript.

**Files to create or modify:**
- None. This task runs the already-built binary; see Acceptance Criterion.

**Key implementation notes:**
- This proof exercises the **entire** pipeline built across this phase: submission
  → validation → persistence → dispatch → real worker execution → status update —
  not a mocked stand-in for any link in that chain.
- Record the literal terminal output in the implementation report; this is what
  `docs/RUNNABLE_PROOF.md`'s Phase 14 entry references.

**Acceptance criterion:**
```bash
cargo build --release -p anvilml --features mock-hardware
./target/release/anvilml &
sleep 2
JOB_ID=$(curl -s -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' \
  -d '{"graph":{"nodes":[{"id":"n0","type":"PassThrough","inputs":{"value":1}}]},"settings":{}}' \
  | python3 -c "import sys,json;print(json.load(sys.stdin)['job_id'])")
sleep 3
curl -s "http://127.0.0.1:8488/v1/jobs/$JOB_ID" \
  | python3 -c "import sys,json; assert json.load(sys.stdin)['status']=='Completed'"
# -> exits 0
kill %1
```

---

## Phase Acceptance Criteria

```bash
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test --workspace --features mock-hardware
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests -v
python -m pytest worker/tests -v -m real_mode

# Runnable Proof (manual):
cargo build --release -p anvilml --features mock-hardware
./target/release/anvilml &
sleep 2
JOB_ID=$(curl -s -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' \
  -d '{"graph":{"nodes":[{"id":"n0","type":"PassThrough","inputs":{"value":1}}]},"settings":{}}' \
  | python3 -c "import sys,json;print(json.load(sys.stdin)['job_id'])")
sleep 3
curl -s "http://127.0.0.1:8488/v1/jobs/$JOB_ID" \
  | python3 -c "import sys,json; assert json.load(sys.stdin)['status']=='Completed'"
# -> exits 0
kill %1
```

---

## Known Constraints and Gotchas

- `JobScheduler`'s internal mutexes must be `tokio::sync::Mutex`, never
  `std::sync::Mutex` — they are held across `await` points when persisting to
  `job_store`, and a `std` mutex held across an await is a correctness bug, not
  just a performance concern.
- The worker-selection algorithm's two steps run in a fixed order —
  `device_preference` always wins over VRAM ranking when both could apply. Don't
  swap or merge these into a single scoring function.
- `PassThrough` is intentionally the only concrete node in the project after this
  phase — the real baseline node set (`LoadModel`, `Sampler`, `VaeDecode`, etc.)
  remains explicitly out of scope, reserved for later, separately-authored phases.
- `AppState` still doesn't hold every `ANVILML_DESIGN.md §13.2` field after this
  phase — `hardware`, `broadcaster`, `artifact_store`, and `env_report` are added
  only when a later task actually needs them.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 14 — Dispatch & Execute

**Capability proved:** A job submitted via `POST /v1/jobs`, referencing the real
`PassThrough` node, is validated, queued, dispatched to a real spawned worker
subprocess via the documented worker-selection algorithm, executed, and reaches
`Completed` — the first genuine end-to-end real dispatch in the project, not a
mocked stand-in for any link in the chain.

\`\`\`bash
# Runnable Proof (manual):
cargo build --release -p anvilml --features mock-hardware
./target/release/anvilml &
sleep 2
JOB_ID=$(curl -s -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' \
  -d '{"graph":{"nodes":[{"id":"n0","type":"PassThrough","inputs":{"value":1}}]},"settings":{}}' \
  | python3 -c "import sys,json;print(json.load(sys.stdin)['job_id'])")
sleep 3
curl -s "http://127.0.0.1:8488/v1/jobs/$JOB_ID" \
  | python3 -c "import sys,json; assert json.load(sys.stdin)['status']=='Completed'"
# -> exits 0
kill %1
\`\`\`
```
