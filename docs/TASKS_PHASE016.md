# Tasks: Phase 16 — Live Events

**Phase:** 16
**Name:** Live Events
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 2, 3, 7, 14, 15

---

## Overview

This phase completes `anvilml-scheduler`'s event loop (subscribing to the full set
of `WorkerEvent`s, not just `ImageReady`, and — critically — persisting terminal job
status for the first time), and exposes the resulting event stream over a real
WebSocket (`GET /v1/events`), including the periodic `SystemStats` background tick.
This is also the phase that fixes a real gap left open since Phase 14: until now,
the dispatch loop only ever set a job's status to `Running` — nothing in the project
processed the worker's response, so a job could never be observed reaching
`Completed`, `Failed`, or `Cancelled` through persisted state, only inferred by its
absence from the queue.

This phase exists right after Artifact Storage Wiring (Phase 15) because both phases
extend the same `event_loop.rs` module Phase 15 created — Phase 15 gave
`ImageReady` its first consumer, and this phase gives every other `WorkerEvent`
variant its first consumer too, while also closing the job-status-persistence gap
that's been latent since Phase 14. The roadmap names "Live Events" as its own group,
distinct from "Cancellation" (the next phase) — this phase covers the broadcast
mechanism and the terminal-status bookkeeping it depends on; cancellation's own IPC
signal path is explicitly out of scope here.

At the start of this phase, `WorkerEvent::Progress`/`Completed`/`Failed`/`Cancelled`
are all defined but unconsumed, and a job's `JobStore` row never reaches a terminal
status. At the end: every worker event is mapped to its `WsEvent` counterpart and
broadcast; job status transitions are correctly persisted and VRAM reservations
correctly released; `GET /v1/events` is a real, working WebSocket endpoint with the
documented initial-frame and lag-disconnect behavior; and `SystemStats` ticks every
five seconds. The phase's Runnable Proof connects a WebSocket client and observes a
real `PassThrough` job's `JobCompleted` event arrive live.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Event loop completion | P16-A1 … P16-A3 | Map every `WorkerEvent` to `WsEvent` and publish; persist terminal job status + release VRAM; restore the worker to `Idle` and wake the dispatch loop |
| B | Server state | P16-B1 | `AppState` gains `broadcaster`, shared with the scheduler's event loop |
| C | WebSocket handler | P16-C1 … P16-C2 | Connection skeleton + initial frame, then the forward loop with lag handling |
| D | Stats tick | P16-D1 | Periodic `SystemStats` background task |
| E | Proof | P16-E1 | The phase's Runnable Proof |

---

## Prerequisites

`event_loop.rs` must already handle `ImageReady` per Phase 15 (P15-C1).
`EventBroadcaster` must exist per Phase 7 (P7-C1). `JobScheduler`'s dispatch loop and
`PassThrough` must work end-to-end per Phase 14.

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §12.4` | P16-A2 | VRAM ledger release on every terminal event |
| `ANVILML_DESIGN.md §12.5` | P16-A3 | The worker-idle-triggered dispatch wake, completing the wake source `P14-A3` deferred |
| `ANVILML_DESIGN.md §16.3` | P16-A1 | Mandatory DEBUG log point for job state transitions |
| `ANVILML_DESIGN.md §13.6` | P16-C1, P16-C2 | The exact WebSocket connect sequence and the 1024-event lag/disconnect rule |
| `ANVILML_DESIGN.md §13.1` | P16-C1, P16-D1 | `ws/` module layout — `mod.rs`, `handler.rs`, `stats_tick.rs` |

---

## Task Descriptions

### Group A — Event loop completion

#### P16-A1: anvilml-scheduler: event_loop subscribes WorkerEvent, publishes WsEvent

**Goal:** Give every remaining `WorkerEvent` variant its first real consumer,
mapping each to the `WsEvent` clients will actually see over the WebSocket.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/event_loop.rs` — adds `spawn_event_loop()`.

**Key implementation notes:**
- The mapping is one-to-one: `Progress`→`JobProgress`, `Completed`→`JobCompleted`,
  `Failed`→`JobFailed`, `Cancelled`→`JobCancelled`. `ImageReady`'s existing save
  logic (Phase 15) is extended to **also** publish `JobImageReady` — but only
  **after** the artifact save succeeds, never before.

**Acceptance criterion:**
```bash
cargo test -p anvilml-scheduler --test event_loop_tests
# -> >=10 tests total in the file, exits 0
```

#### P16-A2: anvilml-scheduler: event_loop updates Job status in JobStore on events

**Goal:** Close the job-status-persistence gap that's existed since Phase 14 —
the first time anything in the project sets a job's status to a terminal state
based on the worker's actual response.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/event_loop.rs` — adds status persistence + VRAM
  release.

**Key implementation notes:**
- Before this task, `GET /v1/jobs/:id` could never observe a `Completed`, `Failed`,
  or `Cancelled` job — dispatch only ever wrote `Running`. This task is what makes
  Phase 14's Runnable Proof (and every later one that checks job status) actually
  correct rather than coincidentally working because the test polled before any
  terminal-state logic existed to contradict it.
- VRAM ledger release happens on **all three** terminal events, per
  `ANVILML_DESIGN.md §12.4` — a job that fails still must release its reservation.
- Restoring the **worker's own** status to `Idle`, and waking the dispatch loop, are
  both explicitly deferred to the next task — this task closes only the job-side
  half of the terminal-event handling.

**Acceptance criterion:**
```bash
cargo test -p anvilml-scheduler --test event_loop_tests
# -> >=15 tests total in the file, exits 0
```

#### P16-A3: anvilml-scheduler: event_loop restores Idle + wakes dispatch loop

**Goal:** Close a real, previously-undetected starvation gap — without this task,
a worker that finishes a job never returns to the `Idle` pool, and even if it did,
nothing would ever wake the dispatch loop to give it more work.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/event_loop.rs` — adds the `Idle` restoration and
  wake call.

**Key implementation notes:**
- Phase 14's `P14-A5` marks a worker `Busy` on dispatch, but nothing reversed that
  transition anywhere in the project until this task — a worker that finished its
  job stayed `Busy` forever, becoming permanently ineligible for future dispatch.
- Phase 14's `P14-A3` explicitly deferred the worker-idle-triggered wake source —
  this task is what that deferred scope actually resolves to. Before this task, the
  dispatch loop's only wake source was `submit()`, meaning a job that arrived while
  every worker was busy could sit `Queued` indefinitely once the queue drained to
  just that one job, with no other event ever prompting a re-check.
- Both the status restoration and the `notify_one()` call happen on **all three**
  terminal events (`Completed`/`Failed`/`Cancelled`) — a failed or cancelled job
  frees its worker exactly as much as a completed one does.

**Acceptance criterion:**
```bash
cargo test -p anvilml-scheduler --test event_loop_tests
# -> >=20 tests total in the file, exits 0
```

---

### Group B — Server state

#### P16-B1: anvilml-server: AppState gains broadcaster field, wired from main.rs

**Goal:** Connect `AppState` to the same `EventBroadcaster` instance the
scheduler's event loop publishes to, so HTTP-layer WebSocket subscribers actually
see the events the scheduler produces.

**Files to create or modify:**
- `crates/anvilml-server/src/state.rs` — adds `broadcaster`.
- `backend/src/main.rs` — constructs one `EventBroadcaster`, shares the same `Arc`
  with both `spawn_event_loop()` and `AppState`.

**Key implementation notes:**
- Sharing the **same** `Arc<EventBroadcaster>` instance is the entire point of this
  task — two separately-constructed broadcasters would silently never see each
  other's events, a subtle bug with no compile-time signal.

**Acceptance criterion:**
```bash
cargo test -p anvilml-server --test state_tests
cargo build -p anvilml
# -> both exit 0
```

---

### Group C — WebSocket handler

#### P16-C1: anvilml-server: ws_handler skeleton + initial SystemStats frame

**Goal:** Implement the WebSocket upgrade and the connection's first frame,
establishing the handler's structure before the ongoing forward loop is added.

**Files to create or modify:**
- `crates/anvilml-server/src/ws/mod.rs`, `ws/handler.rs` — `ws_handler()`,
  `handle_socket()` (partial).

**Key implementation notes:**
- On connect: subscribe to the broadcaster, then immediately send a `SystemStats`
  frame — a placeholder/zero-valued one is acceptable here, since the real periodic
  tick (P16-D1) is a separate, later task.
- The ongoing forward loop is explicitly deferred to the next task — this task
  establishes the connection and sends exactly one frame, then returns.

**Acceptance criterion:**
```bash
cargo test -p anvilml-server --test handler_tests
# -> >=3 tests, exits 0
```

#### P16-C2: anvilml-server: ws_handler forward loop + Lagged disconnect

**Goal:** Complete the handler with the ongoing event-forwarding loop and the
documented behavior for a slow consumer that falls behind the broadcast buffer.

**Files to create or modify:**
- `crates/anvilml-server/src/ws/handler.rs` — adds the forward loop.

**Key implementation notes:**
- Every subsequent `WsEvent` is forwarded as a JSON text frame.
- On `RecvError::Lagged` (the consumer fell more than 1024 events behind), the
  connection is **closed**, not caught up — per `ANVILML_DESIGN.md §13.6`'s explicit
  rule, the client must reconnect rather than receive a gapped or replayed stream.

**Acceptance criterion:**
```bash
cargo test -p anvilml-server --test handler_tests
# -> >=7 tests total in the file, exits 0
```

---

### Group D — Stats tick

#### P16-D1: anvilml-server: stats_tick.rs background SystemStats every 5s

**Goal:** Implement the periodic background task that gives every connected
WebSocket client a regular heartbeat of system state, not just job-related events.

**Files to create or modify:**
- `crates/anvilml-server/src/ws/stats_tick.rs` — `spawn_stats_tick()`.
- `backend/src/main.rs` — calls `spawn_stats_tick()` alongside the dispatch and
  event loops.

**Key implementation notes:**
- The tick interval is injected as a constructor parameter, not a hardcoded
  `Duration::from_secs(5)` literal — the same testability pattern Phase 8's
  `keepalive.rs` established, letting tests use millisecond-scale intervals.

**Acceptance criterion:**
```bash
cargo test -p anvilml-server --features mock-hardware --test stats_tick_tests
# -> >=4 tests, exits 0
```

---

### Group E — Proof

#### P16-E1: Runnable Proof: WebSocket client observes JobCompleted for PassThrough job

**Goal:** Produce this phase's Runnable Proof — the first genuine end-to-end
demonstration of the live event stream, not just REST polling.

**Files to create or modify:**
- None. This task runs the already-built binary plus a short test script; see
  Acceptance Criterion.

**Key implementation notes:**
- This proof exercises the entire chain built across Phases 14–16: submission →
  dispatch → real worker execution → event publication → WebSocket delivery — all
  observed live, not inferred from polling a REST endpoint after the fact.

**Acceptance criterion:**
```bash
# A short Python script using the websockets library:
# 1. Connects to ws://127.0.0.1:8488/v1/events
# 2. Submits a PassThrough job via a parallel HTTP POST to /v1/jobs
# 3. Asserts a job_completed JSON frame with the matching job_id arrives within 10s
# -> script exits 0
```

---

## Phase Acceptance Criteria

```bash
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test --workspace --features mock-hardware

# Runnable Proof (manual): see P16-E1 — a WebSocket client observes a real
# JobCompleted event for a submitted PassThrough job, delivered live over
# /v1/events, within 10 seconds of submission.
```

---

## Known Constraints and Gotchas

- Before this phase, no job could ever be observed reaching a terminal status —
  this was a real, latent gap since Phase 14, not a regression introduced here.
  P16-A2 is what actually fixes it.
- A second, separate gap existed alongside the first: nothing anywhere transitioned
  a worker back to `Idle` after finishing a job, and nothing woke the dispatch loop
  on that transition — a worker that finished its job stayed permanently `Busy`,
  and a queued job with no other submission to trigger a wake could starve
  indefinitely. `P16-A3` is what closes this — found on review, not introduced as
  a regression here either.
- `EventBroadcaster` must be the **same shared instance** between the scheduler's
  event loop and `AppState` — two independently constructed broadcasters would
  silently never communicate, with no error to surface the mistake.
- A `Lagged` WebSocket consumer is disconnected, never caught up — this is a
  deliberate design choice from `ANVILML_DESIGN.md §13.6`, not a limitation to work
  around with a larger buffer or a replay mechanism.
- `stats_tick`'s interval must remain an injectable parameter — hardcoding
  `Duration::from_secs(5)` directly in the spawned task would make its own tests
  either slow or impossible to write deterministically.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 16 — Live Events

**Capability proved:** A WebSocket client connected to `GET /v1/events` observes a
real `JobCompleted` event for a `PassThrough` job submitted via `POST /v1/jobs`,
delivered live as the job actually completes — not inferred by polling a REST
endpoint afterward.

\`\`\`bash
cargo build --release -p anvilml --features mock-hardware
./target/release/anvilml &
sleep 1
python3 - <<'EOF'
import asyncio, json, urllib.request
import websockets

async def main():
    async with websockets.connect("ws://127.0.0.1:8488/v1/events") as ws:
        await ws.recv()  # initial SystemStats frame
        req = urllib.request.Request(
            "http://127.0.0.1:8488/v1/jobs",
            data=json.dumps({
                "graph": {"nodes": [{"id": "n0", "type": "PassThrough", "inputs": {"value": 1}}]},
                "settings": {}
            }).encode(),
            headers={"Content-Type": "application/json"},
        )
        job_id = json.loads(urllib.request.urlopen(req).read())["job_id"]
        async with asyncio.timeout(10):
            while True:
                frame = json.loads(await ws.recv())
                if frame.get("type") == "job_completed" and frame.get("job_id") == job_id:
                    return

asyncio.run(main())
EOF
# -> script exits 0; a job_completed frame with the matching job_id arrived within 10s
kill %1
\`\`\`
```
