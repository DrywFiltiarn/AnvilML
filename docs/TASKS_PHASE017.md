# Tasks: Phase 17 — Cancellation

**Phase:** 17
**Name:** Cancellation
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 2, 3, 7, 9, 10, 14, 16

---

## Overview

This phase implements cooperative job cancellation across both halves of the
system: `JobScheduler::cancel()` branches correctly on a job's current status
(immediate for `Queued`, an IPC signal for `Running`, a no-op for anything already
terminal), and the Python worker gains its first real graph executor —
`executor.py` — which checks the cooperative `cancel_flag` between node execution
steps. `POST /v1/jobs/:id/cancel` exposes this over HTTP with the documented
202/409/404 status code split.

This phase exists right after Live Events (Phase 16) because cancellation's
`Running` path depends on the event loop correctly processing the worker's
eventual `Cancelled` response — Phase 16 is what made that processing real.
Cancellation also requires `executor.py` to exist at all: until now, `PassThrough`
(Phase 14) was invoked directly without a graph executor, since a single-node graph
needs no topological ordering. This phase is the first to build a genuine
multi-node-capable execution loop, specifically because cancellation needs a defined
"between steps" checkpoint to cooperate at.

At the start of this phase, `cancel()` is a thin delegate to the in-memory queue
only (Phase 14), with no awareness of `Running` jobs, and no `executor.py` exists.
At the end: cancelling a `Queued` job is immediate and IPC-free; cancelling a
`Running` job sends a cooperative `CancelJob` signal the worker's executor checks
between node steps; cancelling an already-terminal job is a no-op, not an error; and
`POST /v1/jobs/:id/cancel` returns the correct status code for each case.

This revision adds `P17-B3`/`P17-B4` to the phase's original task set, found by
a forward trace of `execute_graph()` (`P17-B2`) against every later phase's task
`context` for an actual call site — the same method that found the
`create_pool()`/`SeedLoader` gap in Phase 6 and the `RespawnPolicy` gap in
Phase 8. The finding: `worker_main.py`'s message dispatch loop, left as a
placeholder by Phase 9's `P9-D2` ("logs and continues"), was originally only
ever extended once in this phase's own task set — by the `CancelJob` handler
(now `P17-B5`, originally authored as this phase's third task), exclusively for
`CancelJob`. No task anywhere handled `WorkerMessage::Execute`, despite the
Rust scheduler genuinely sending it (Phase 14's `P14-A4`) and the Rust event
loop genuinely being ready to receive `WorkerEvent::Completed`/`Failed` for it
(Phase 16's `P16-A1`/`P16-A2`). As originally authored, every later phase's
`POST /v1/jobs`-based Runnable Proof (`P24-F1`, `P25-F1`, `P26-D1`) would have
submitted a job that dispatched correctly on the Rust side and then hung in
`Running` forever, since the Python worker never called any node's `execute()`
for it. `P17-B3` wires the `Execute` message to `execute_graph()` on a
background thread and sends `Completed` on success; `P17-B4` adds the
companion failure path, sending `Failed` instead of leaving the job silently
hung. The original `CancelJob` handler task was renumbered from `P17-B3` to
`P17-B5` to make room — its `prereqs` were updated from `P17-B2` to `P17-B4`
accordingly, since cancellation only makes sense once a job can actually be
running with a terminal-event path already in place. `P17-C1`'s `prereqs` were
updated to reference `P17-B5` (its original dependency, just renumbered), not
the new `P17-B3`.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Scheduler-side cancel | P17-A1 … P17-A2 | Status-branching `cancel()`, then the IPC `CancelJob` send for `Running` jobs |
| B | Worker-side cancel | P17-B1 … P17-B5 | `executor.py`'s topological sort, its cancel-checking execution loop, the `Execute`-message handler (success then failure path), then `worker_main.py`'s `CancelJob` handling |
| C | HTTP handler | P17-C1 | `POST /v1/jobs/:id/cancel` |
| D | Proof | P17-D1 | The phase's Runnable Proof |

---

## Prerequisites

`JobScheduler::cancel()` must exist (currently queue-only) per Phase 14 (P14-A2).
`event_loop.rs` must correctly persist `Cancelled` status per Phase 16 (P16-A2).
`NodeContext.cancel_flag` must exist per Phase 10 (P10-A3). `worker_main.py`'s
dispatch loop placeholder must exist per Phase 9.

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §20` Cancellation entry | P17-A1, P17-A2 | "Cooperative cancel: `Queued` (immediate) and `Running` (IPC signal)" — the exact split this phase implements |
| `ANVILML_DESIGN.md §14.5` | P17-B2, P17-B3 | `NodeContext.cancel_flag`'s exact semantics — a `threading.Event` checked cooperatively, never a forceful interrupt |
| `ANVILML_DESIGN.md §13.4`, §13.5 | P17-C1 | Exact route and status code mapping (`202`/`409`/`404`) |

---

## Task Descriptions

### Group A — Scheduler-side cancel

#### P17-A1: anvilml-scheduler: JobScheduler::cancel() dispatches by current status

**Goal:** Replace the queue-only `cancel()` from Phase 14 with status-aware
branching, handling the `Queued` and terminal-state cases completely in this task.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/scheduler.rs` — `cancel()` gains status branching.

**Key implementation notes:**
- `Queued`: the lazy-removal queue cancel (Phase 13) is sufficient — the job never
  reached a worker, so `status=Cancelled` is set and persisted immediately, with no
  IPC involved at all.
- Any already-terminal status (`Completed`/`Failed`/`Cancelled`) returns `Ok(false)`
  — cancelling a finished job is a no-op, not an error, per the idempotent-cancel
  principle.
- `Running` is explicitly only stubbed to return `Ok(true)` in this task — the
  actual IPC send is the next task's scope.

**Acceptance criterion:**
```bash
cargo test -p anvilml-scheduler --test scheduler_tests
# -> >=5 tests, exits 0
```

#### P17-A2: anvilml-scheduler: cancel() sends WorkerMessage::CancelJob for Running jobs

**Goal:** Complete the `Running` branch with the actual cooperative IPC signal.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/scheduler.rs` — sends `CancelJob`.

**Key implementation notes:**
- This is **cooperative, not forceful** — the scheduler never kills the worker
  process or assumes the cancellation took immediate effect. The worker decides when
  it's safe to actually stop.
- `status` stays `Running` immediately after `cancel()` returns — only the event
  loop (Phase 16) transitions it to `Cancelled`, once the worker's own `Cancelled`
  event actually arrives.

**Acceptance criterion:**
```bash
cargo test -p anvilml-scheduler --test scheduler_tests
# -> >=9 tests total in the file, exits 0
```

---

### Group B — Worker-side cancel

#### P17-B1: worker/executor.py: topological sort of node graph

**Goal:** Create the project's first real graph executor module, starting with
just the ordering logic, before any execution or cancellation-checking is added.

**Files to create or modify:**
- `worker/executor.py` — new; `topo_sort()`.

**Key implementation notes:**
- This module didn't exist before this phase — `PassThrough` (Phase 14) was a
  single node, invoked directly with no need for topological ordering.
- This is a separate Python implementation of a Kahn's-algorithm-style sort,
  conceptually similar to `anvilml-scheduler`'s Rust-side `validate_graph()`
  (Phase 12) but not literally shared code across the language boundary.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_executor.py -v
# -> >=4 tests, exits 0
```

#### P17-B2: worker/executor.py: execute_graph loop with cancel_flag check

**Goal:** Complete the executor with the actual execution loop and the
cooperative cancellation checkpoint — the entire reason this module exists in this
phase.

**Files to create or modify:**
- `worker/executor.py` — adds `execute_graph()`.

**Key implementation notes:**
- `ctx.cancel_flag.is_set()` is checked **before** starting each node's `execute()`
  call — cooperative, between steps. A node already mid-`execute()` is never
  interrupted mid-step.
- This receives exactly the scope P17-B1 deferred — confirm `topo_sort()` exists and
  produces a valid ordering before building the execution loop on top of it.

**Acceptance criterion:**
```bash
python -m pytest worker/tests/test_executor.py -v
# -> >=9 tests total in the file, exits 0
```

#### P17-B3: worker/worker_main.py: dispatch loop handles WorkerMessage::Execute, success path

**Goal:** Close an audit-found gap — `execute_graph()` (`P17-B2`) is never called
for a real job anywhere in the original task set. The Rust side genuinely sends
`WorkerMessage::Execute` (Phase 14's `P14-A4`) and the Rust event loop genuinely
handles `WorkerEvent::Completed` (Phase 16's `P16-A1`/`P16-A2`), but nothing on
the Python side ever receives `Execute` and calls `execute_graph()` — the
dispatch loop placeholder `P9-D2` left behind was only ever extended once in
the original task set, by this phase's `CancelJob` handler (`P17-B5` below),
and exclusively for `CancelJob`.

**Files to create or modify:**
- `worker/worker_main.py` — adds an `Execute` branch to the dispatch loop.

**Key implementation notes:**
- Build a `NodeContext`-producing `ctx_factory` bound to this job's `job_id`
  (Phase 10's `P10-A3`) and call `execute_graph(msg["graph"], ctx_factory)`
  (`P17-B2`) on a background thread — the dispatch loop itself must stay
  responsive to an incoming `CancelJob` while a job is running, which a
  synchronous call would block.
- On success, send `WorkerEvent::Completed{job_id, elapsed_ms}`.
- Failure-path handling (an unhandled exception escaping `execute_graph()`) is
  `P17-B4`'s scope, deferred here — do not add a catch-all `except` clause in
  this task.
- Does not change `execute_graph()`'s own signature or `cancel_flag` behavior —
  `P17-B1`/`P17-B2` are unmodified by this task.

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py -v
# -> >=4 tests, exits 0: Execute triggers execute_graph() with a job-scoped
#    ctx_factory, success sends Completed with a real elapsed_ms, the dispatch
#    loop stays responsive to CancelJob sent during execution
```

#### P17-B4: worker/worker_main.py: Execute handler failure path sends WorkerEvent::Failed

**Goal:** Complete the `Execute` handler with the failure path `P17-B3`
deferred — without this, a node that raises during a real job leaves it hung
forever with no terminal event, which is the exact failure mode the audit
finding describes.

**Files to create or modify:**
- `worker/worker_main.py` — adds an outer exception catch around `P17-B3`'s
  background-thread execution.

**Key implementation notes:**
- When `execute_graph()` (or its background thread) raises an unhandled
  exception, catch it at the dispatch loop's outer level and send
  `WorkerEvent::Failed{job_id, error: str(exc), traceback: <formatted
  traceback>}` instead of leaving the job silently hung.
- Does not change `execute_graph()`'s own per-node error handling — only the
  dispatch loop's outer catch around the background thread `P17-B3`
  introduced.

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py -v
# -> >=3 new tests (>=7 total in file): a node raising inside execute_graph()
#    results in Failed being sent (not Completed, not silence), error contains
#    the exception message, traceback is populated and non-empty
```

#### P17-B5: worker/worker_main.py: dispatch loop handles WorkerMessage::CancelJob

**Goal:** Connect the supervisor's `CancelJob` message to the executor's
`cancel_flag`, completing the worker side of the cooperative cancellation chain.
Sequenced after `P17-B3`/`P17-B4` since cancellation only makes sense once a
job can actually be running (`P17-B3`'s `Execute` handler) with a terminal-event
path already in place (`P17-B4`'s failure handling) — cancel itself sends a
third terminal event, `Cancelled`, into the same dispatch loop these two tasks
established.

**Files to create or modify:**
- `worker/worker_main.py` — extends the dispatch loop's `CancelJob` branch
  (the placeholder `P9-D2` left, distinct from the `Execute` branch `P17-B3`
  added).

**Key implementation notes:**
- A `CancelJob` for a `job_id` that doesn't match the currently-executing job is
  logged at `DEBUG` and ignored — not an error; this can legitimately happen if the
  job already completed before the cancel message arrived.
- When the executor actually stops due to the flag, `WorkerEvent::Cancelled` is sent
  back to the supervisor — this is what Phase 16's event loop eventually persists.

**Acceptance criterion:**
```bash
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/test_worker_main.py -v
# -> >=4 new tests (>=11 total in file)
```

---

### Group C — HTTP handler

#### P17-C1: anvilml-server: POST /v1/jobs/:id/cancel handler

**Goal:** Expose cancellation over HTTP with the exact three-way status code split
the design document specifies.

**Files to create or modify:**
- `crates/anvilml-server/src/handlers/jobs.rs` — adds `cancel_job()`.

**Key implementation notes:**
- `202` on `Ok(true)`, `409` on `Ok(false)` (already terminal), `404` via
  `AnvilError::JobNotFound` if the ID doesn't exist at all.
- This 404-vs-409 split requires `cancel()` to actually distinguish "not found" from
  "found but already terminal" — confirm this distinction exists in the scheduler's
  return type before wiring the handler; if it currently conflates the two, flag as
  a Deviation and fix `cancel()`'s signature rather than papering over it in the
  handler.

**Acceptance criterion:**
```bash
cargo test -p anvilml-server --test jobs_tests
# -> >=14 tests total in the file, exits 0
```

---

### Group D — Proof

#### P17-D1: Runnable Proof: cancelling a Queued job returns 202 then 409 on retry

**Goal:** Produce this phase's Runnable Proof, demonstrating both the success path
and the idempotent-cancel rejection live against a real server.

**Files to create or modify:**
- None. This task runs the already-built binary; see Acceptance Criterion.

**Key implementation notes:**
- `ANVILML_MOCK_NODE_DELAY_MS` is set high enough to keep the job observably
  `Queued`/`Running` for the first cancel call, so the proof exercises a real
  in-flight cancellation rather than racing against instant completion.

**Acceptance criterion:**
```bash
cargo build --release -p anvilml --features mock-hardware
./target/release/anvilml &
sleep 1
JOB_ID=$(curl -s -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' \
  -d '{"graph":{"nodes":[{"id":"n0","type":"PassThrough","inputs":{"value":1}}]},"settings":{}}' \
  | python3 -c "import sys,json;print(json.load(sys.stdin)['job_id'])")
curl -s -o /dev/null -w '%{http_code}' -X POST "http://127.0.0.1:8488/v1/jobs/$JOB_ID/cancel"
# -> 202
curl -s -o /dev/null -w '%{http_code}' -X POST "http://127.0.0.1:8488/v1/jobs/$JOB_ID/cancel"
# -> 409
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

# Runnable Proof (manual): see P17-D1 — a Queued job's first cancel returns 202;
# a second cancel on the same (now-Cancelled) job returns 409.
```

---

## Known Constraints and Gotchas

- Cancellation is **cooperative**, never forceful — the scheduler never kills a
  worker process to enforce a cancel; it only sends a signal the worker's executor
  checks between steps.
- `cancel()`'s three-way outcome (`Queued`-immediate, `Running`-IPC,
  terminal-no-op) must be distinguishable from "not found" for the HTTP handler's
  404-vs-409 split to work correctly — this distinction is load-bearing, not
  incidental.
- `executor.py`'s cancel check happens **between** node steps, never interrupting a
  node already mid-`execute()` — a node's own internal loop (if any) is responsible
  for its own finer-grained cooperation, which is out of this phase's scope.
- A `CancelJob` for a non-matching `job_id` is expected, normal behavior (a race
  between job completion and the cancel message), not an error condition to log at
  `WARN` or above.
- **`P17-D1`'s own Runnable Proof was, until `P17-B3`/`P17-B4` were added,
  silently broken by the gap those two tasks fix** — not exempt from it. Its
  acceptance criterion sets `ANVILML_MOCK_NODE_DELAY_MS` specifically to keep
  the job briefly `Running` before cancelling, and per `ENVIRONMENT.md §10.6`,
  mock mode is not a separate, simpler code path — `ANVILML_WORKER_MOCK`
  selects between two equally-maintained branches of the same `execute_graph()`
  call, both of which need the `Execute`-message handler `P17-B3` adds to ever
  run at all. As originally authored (before this revision), `P17-D1`'s job
  would never have reached `Running` in any observable sense — the dispatch
  loop would have logged and continued on the incoming `Execute` message,
  leaving the job permanently `Queued` in the worker's view even though the
  Rust scheduler believed it had been dispatched, and the cancel call would
  likely still have returned `202` (since `JobScheduler::cancel()`, Phase 17's
  own `P17-A1`/`P17-A2`, only checks the Rust-side `Job.status`, not whether
  the Python worker is actually processing it) without ever proving the
  underlying execution path was real. This is exactly the kind of
  internally-consistent-but-not-actually-exercising-the-real-path failure mode
  this document's verification methodology exists to catch.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 17 — Cancellation

**Capability proved:** A `Queued` job's first `POST /v1/jobs/:id/cancel` call
returns `202`; a second cancel call on the same now-`Cancelled` job returns `409` —
demonstrating both the success path and idempotent-cancel rejection against a live
server.

\`\`\`bash
# Runnable Proof (manual):
cargo build --release -p anvilml --features mock-hardware
./target/release/anvilml &
sleep 1
JOB_ID=$(curl -s -X POST http://127.0.0.1:8488/v1/jobs -H 'Content-Type: application/json' \
  -d '{"graph":{"nodes":[{"id":"n0","type":"PassThrough","inputs":{"value":1}}]},"settings":{}}' \
  | python3 -c "import sys,json;print(json.load(sys.stdin)['job_id'])")
curl -s -o /dev/null -w '%{http_code}' -X POST "http://127.0.0.1:8488/v1/jobs/$JOB_ID/cancel"
# -> 202
curl -s -o /dev/null -w '%{http_code}' -X POST "http://127.0.0.1:8488/v1/jobs/$JOB_ID/cancel"
# -> 409
kill %1
\`\`\`
```