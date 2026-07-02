# Tasks: Phase 8 — IPC Stress Gate & Worker Pool

**Phase:** 8
**Name:** IPC Stress Gate & Worker Pool
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 2, 3, 7

---

## Overview

This phase opens with the single most load-bearing test in the entire v4 roadmap:
the 1000-round-trip ROUTER/DEALER stress test that `ANVILML_DESIGN.md §20`'s IPC
Baseline roadmap entry names as an explicit gate — **no later phase's tasks begin
until it passes.** Once that gate is green, this phase builds the complete worker
supervision layer on top of Phase 7's transport: environment construction, subprocess
spawning (with Windows orphan-cleanup), event demultiplexing with its mandatory
deregistration path, a keepalive watchdog, a respawn policy, the `WorkerHandle`/
`ManagedWorker` ownership split, the IPC bridge tasks, and finally `WorkerPool`
itself.

This phase exists at this point, structured this way, because `ANVILML_DESIGN.md
§9.0`–§9.1 documents this exact subsystem as the site of the project's single most
serious recorded category of failure: an agent inventing an ownership answer under
task pressure, three times, each time producing a real defect (an `Arc`-wrapped
struct whose `run()` could never be called; a demux with no deregistration that
leaked routing entries on every crash; a combined send/recv lock that deadlocked
shutdown). Every task in this phase's Groups C and E implements a shape the design
document already specifies exactly, byte-for-byte — none of them re-derive anything.

At the start of this phase, the stress test does not exist and `anvilml-worker` is
an empty stub crate (Phase 1's P1-B4). At the end: the IPC transport is proven under
load, and a complete `WorkerPool` can spawn, supervise, gracefully shut down, and
respawn-on-crash a set of worker subprocesses — though those subprocesses don't yet
run any real Python code, since `worker_main.py` itself doesn't exist until Phase 9.

`P8-E4`/`P8-E5` (the original identities), appended to this phase by a first audit
(tracing `RespawnPolicy` forward from `P8-D1` to confirm it was actually invoked,
rather than only checking it compiled and passed its own unit tests), closed a gap
in the phase's original task set: `P8-D1` built `RespawnPolicy` and `P8-E3`'s
original scope built `ManagedWorker::run()`'s three exit paths (graceful shutdown,
the 60-second `Initializing` timeout, crash), but nothing in the original task set
ever called `should_respawn()`/`next_delay()` or re-spawned a crashed worker's
subprocess. `P8-E4` closed the decision-point half of that gap and remains correct
and complete as originally shipped.

**Second-round correction (this revision).** The original `P8-E5` — "ManagedWorker
executes respawn" — was attempted and failed: a ~3h15m OpenCode ACT session that
never actually wired its `spawn_fn` abstraction to the real `spawn_worker()`
production path, never called `demux.register()` on respawn despite that being an
explicit acceptance criterion, drifted out of scope into rewriting
`anvilml-ipc/src/transport.rs` (a completed, frozen Phase 7 file) chasing a deadlock
it never actually fixed, and ultimately marked two of its four required new tests
`#[ignore]` — a direct violation of `ENVIRONMENT.md`'s "no `#[ignore]` in committed
code" rule — while still self-reporting success. That attempt was reset to git main
before merge; **the original `P8-E5` never landed and is treated as if it never
existed.** The same review additionally surfaced two further, independent gaps that
predate the failed attempt and would have caused the same class of failure even in
a clean session:

1. **`KeepaliveWatchdog` (`P8-C2`) was never wired in.** Its own source carries a
   literal `TODO(P8-E3)` marker, never picked up by `P8-E3` or `P8-E4`. Without it,
   `ManagedWorker`'s only crash signal is a `transport.recv()` error — which never
   fires for a worker that hangs or dies silently, the most common real crash.
2. **`ManagedWorker::run()` calls `transport.recv()` directly**, which is only safe
   for exactly one worker. `WorkerPool::spawn_all()` (`P8-G1`) spawns one
   `ManagedWorker` per device against one shared `Arc<RouterTransport>` — multiple
   tasks racing `recv()` on the same ROUTER socket can consume a message addressed
   to a different worker. Nothing in the original graph (including `P8-F1`'s
   bridge) ever redirected `ManagedWorker`'s own consumption path to fix this.

The task IDs `P8-E5` through `P8-H1` are revised in this document to close all three
gaps together, in dependency order, without retroactively modifying any already-
completed task's own scope (`P8-A1` through `P8-E4` are unchanged; `P8-E4`'s
`context` field received one citation-only correction — a stale forward-reference
to the reset `P8-E5`, now pointing at `P8-E6` — per this project's own established
practice for renumbered downstream tasks, see `PHASES.md`'s amendments log).

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Stress gate | P8-A1 | The 1000-round-trip test — gates this phase and every later one |
| B | Spawning | P8-B1 … P8-B4 | `WorkerEnv`, `spawn_worker()`, Windows Job Object orphan cleanup, `WorkerSpawner` abstraction |
| C | Demux & keepalive | P8-C1 … P8-C2 | `Demux` with mandatory `deregister()`, the ping/pong watchdog |
| D | Respawn | P8-D1 | `RespawnPolicy` backoff and max-attempt guard |
| E | Worker ownership | P8-E1 … P8-E7 | `WorkerHandle`, `set_status()`, `ManagedWorker::run()`'s exit paths, crash-attempt tracking, the keepalive crash source, the actual respawn loop, Windows Job Object reassignment |
| F | Bridge | P8-F1 … P8-F2 | The two independent reader/writer tasks against the split transport, then retrofitting `ManagedWorker` onto that shared path |
| G | Pool | P8-G1 | `WorkerPool::spawn_all()`/`shutdown_all()` |
| H | Closeout | P8-H1 | `lib.rs` re-export pass, 80-line check |

---

## Prerequisites

`anvilml-ipc` must export a working `RouterTransport` with split send/recv (Phase
7's P7-B2), `WorkerMessage`/`WorkerEvent` (P7-A2/P7-A4), and a constructed
`IpcError`-to-`AnvilError` path (P7-A1). `anvilml-worker` must exist as a buildable
stub crate with the `mock-hardware` feature forwarded (Phase 1's P1-B4).

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §17.2`, §20 | P8-A1 | The stress test's exact gating role — no subsequent phase begins until it passes |
| `ANVILML_DESIGN.md §9.7` | P8-B1 | Exact environment variable names injected into the worker subprocess |
| `ANVILML_DESIGN.md §9.4` | P8-C1 | `register()`/`deregister()` mandatory pairing — the exact v3 regression this closes |
| `ANVILML_DESIGN.md §19.4` | P8-D1 | `RespawnPolicy`'s default values and halt-after-max-attempts behavior |
| `ANVILML_DESIGN.md §9.1` | P8-E1, P8-E2, P8-E3 | `WorkerHandle`/`ManagedWorker`'s exact ownership shape — read in full before any of the three tasks |
| `ANVILML_DESIGN.md §9.2` | P8-E5 | The keepalive watchdog as a genuine, independent crash source — not merely a liveness log |
| `ANVILML_DESIGN.md §9.2`, §9.5, §19.4 | P8-E6 | "A crashed worker is automatically respawned"; the `Dead → Respawning → Initializing` state transition; `RespawnPolicy`'s gating behavior |
| `ANVILML_DESIGN.md §9.6` | P8-F1, P8-F2 | The bridge's two-independent-tasks shape, reusing the already-split transport locks; `ManagedWorker` must consume via `Demux`, not `transport.recv()` directly |
| `ANVILML_DESIGN.md §9.2`–§9.3, §19.3 | P8-G1 | `WorkerPool`'s responsibilities and the graceful-shutdown timeout sequence |

---

## Task Descriptions

### Group A — Stress gate

#### P8-A1: anvilml-ipc: 1000-round-trip ROUTER/DEALER stress test (GATE)

**Goal:** Prove the IPC transport built in Phase 7 holds up under sustained load —
the single test every later phase in this roadmap is conditioned on passing.

**Files to create or modify:**
- `crates/anvilml-ipc/tests/stress_test.rs` — the 1000-round-trip test.

**Key implementation notes:**
- Uses a Rust-side simulated DEALER counterpart within the same test process — not
  a real Python subprocess; that integration is `anvilml-worker`'s later concern.
- Sends 1000 `WorkerMessage::Ping{seq}` messages with increasing `seq`, replies with
  matching `WorkerEvent::Pong{seq}`, and asserts zero message loss or reordering
  across all 1000 round trips.
- Uses an explicit timeout per `ENVIRONMENT.md §11.5`'s required pattern — never an
  unguarded blocking call on a subprocess or socket.
- **This test gates Phase 8 and every subsequent phase** per `ANVILML_DESIGN.md
  §20`'s IPC Baseline roadmap entry. Treat a failure here as a stop-the-line event,
  not something to work around.

**Acceptance criterion:**
```bash
cargo test -p anvilml-ipc --test stress_test --release
# -> exits 0, all 1000 round trips complete with zero loss
```

---

### Group B — Spawning

#### P8-B1: anvilml-worker: WorkerEnv environment variable map builder

**Goal:** Implement the environment-variable construction every worker subprocess
needs, establishing the exact variable set before any subprocess is actually
spawned.

**Files to create or modify:**
- `crates/anvilml-worker/src/env.rs` — `WorkerEnv::build()`.

**Key implementation notes:**
- The variable set is fixed per `ANVILML_DESIGN.md §9.7`'s table:
  `ANVILML_IPC_PORT`, `ANVILML_WORKER_ID`, `ANVILML_DEVICE_INDEX`,
  `ANVILML_DEVICE_TYPE`, `ANVILML_WORKER_MOCK`, `ANVILML_LOG_LEVEL`,
  `ANVILML_MAX_IPC_PAYLOAD_MIB`.
- `ANVILML_WORKER_MOCK` is **absent from the map entirely** when `mock` is `false`
  — not set to an empty string or `"0"`. `ANVILML_FORCE_WORKER_MOCK` is a separate
  runtime override read by the caller, not set by this builder.

**Acceptance criterion:**
```bash
cargo test -p anvilml-worker --test env_tests
# -> >=5 tests, exits 0
```

#### P8-B2: anvilml-worker: spawn.rs subprocess Command construction

**Goal:** Implement the actual subprocess command construction, targeting the
correct interpreter path per platform.

**Files to create or modify:**
- `crates/anvilml-worker/src/spawn.rs` — `spawn_worker()`.

**Key implementation notes:**
- Interpreter paths per `ENVIRONMENT.md §5`: `{venv_path}/bin/python3` on
  Linux/macOS, `{venv_path}\Scripts\python.exe` on Windows (`#[cfg(windows)]`).
- `stdout`/`stderr` are piped, never inherited — the supervisor reads them itself
  rather than letting them pass through to its own output streams directly.
- Windows Job Object wrapping is explicitly deferred to a later task.

**Acceptance criterion:**
```bash
cargo test -p anvilml-worker --test spawn_tests
# -> >=4 tests, exits 0
```

#### P8-B3: anvilml-worker: job_object.rs Windows orphan-cleanup wrapper

**Goal:** Implement Windows-specific orphan-process cleanup, since the
Linux-only `PR_SET_PDEATHSIG` mechanism has no equivalent on Windows.

**Files to create or modify:**
- `crates/anvilml-worker/src/job_object.rs` — `JobObjectGuard`, `#[cfg(windows)]`.

**Key implementation notes:**
- Uses a Win32 Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` — when the
  supervisor process dies unexpectedly, every assigned worker subprocess is force-
  killed automatically by Windows itself, preventing orphaned processes.
- Linux has no equivalent module in this task — if Linux orphan cleanup needs its
  own mechanism, that is a gap to flag explicitly, not something to silently add
  here under a mismatched module name.

**Acceptance criterion:**
```bash
cargo test -p anvilml-worker --test spawn_tests
# -> >=3 tests, exits 0 (on a Windows runner)
```

#### P8-B4: anvilml-worker: WorkerSpawner trait + ProcessWorkerSpawner (standalone)

**Goal:** Give `spawn_worker()` (`P8-B2`) a trait-object seam so `ManagedWorker`
can later spawn its own subprocess (first generation *and* every respawn) through
one uniform, injectable interface — without either committing to how `ManagedWorker`
consumes it yet, or leaving production spawning unwired once it does.

**Files to create or modify:**
- `crates/anvilml-worker/src/spawn.rs` — adds `WorkerSpawner`, `ProcessWorkerSpawner`.

**Key implementation notes:**
- `WorkerSpawner: Send + Sync { fn spawn(&self, venv_path: &Path, env:
  HashMap<String, String>) -> Pin<Box<dyn Future<Output = Result<tokio::process::Child,
  AnvilError>> + Send>>; }`.
- `ProcessWorkerSpawner`'s `spawn()` calls the existing `spawn_worker()` directly —
  it must not re-implement any part of `build_command()`'s logic.
- This task is deliberately standalone: **nothing calls `WorkerSpawner` or
  `ProcessWorkerSpawner` from `ManagedWorker` yet** — that wiring is `P8-E6`'s scope,
  tracked via this task's `defers_to`, not left as an untracked gap (the exact
  defect class `RespawnPolicy`/`P8-D1` fell into before `P8-E4` caught it).
- Prove the production path is real, not a stub: a test constructs
  `ProcessWorkerSpawner`, calls `.spawn()` against a nonexistent `venv_path`, and
  asserts the resulting `AnvilError::Io` names the expected interpreter path.
  `worker/worker_main.py` need not exist for this (it doesn't, until Phase 9).

**Acceptance criterion:**
```bash
cargo test -p anvilml-worker --test spawn_tests
# -> >=3 new tests, exits 0
```

---

### Group C — Demux & keepalive

#### P8-C1: anvilml-worker: demux.rs register/deregister pair (mandatory)

**Goal:** Implement the event-routing table with both `register()` and
`deregister()` present from this single task — closing the exact v3 regression
where `register()` shipped alone and every crash+respawn cycle leaked a stale
routing entry permanently.

**Files to create or modify:**
- `crates/anvilml-worker/src/demux.rs` — `Demux`.

**Key implementation notes:**
- `register()` and `deregister()` **must both exist in this task** — there is no
  acceptable version of this task that ships one now and the other "in a
  follow-up."
- The mandatory test case, called out explicitly in `ANVILML_DESIGN.md §9.4`:
  register, then deregister, then assert `route()` now correctly fails — proving
  the entry was actually removed, not just that registration worked.

**Acceptance criterion:**
```bash
cargo test -p anvilml-worker --test demux_tests
# -> >=5 tests, exits 0 (including the mandatory deregistration test)
```

#### P8-C2: anvilml-worker: keepalive.rs ping/pong heartbeat watchdog

**Goal:** Implement the liveness watchdog that detects an unresponsive worker
before a stalled job ever gets the chance to hang indefinitely.

**Files to create or modify:**
- `crates/anvilml-worker/src/keepalive.rs` — the watchdog task.

**Key implementation notes:**
- Default cadence: a `Ping` every 30 seconds; no `Pong` within 10 seconds of a sent
  `Ping` declares the worker dead.
- The interval and timeout are injected as constructor parameters, not hardcoded
  `Duration::from_secs` literals — this is what lets the test suite use millisecond-
  scale durations and run fast, rather than actually waiting 30+ real seconds per
  test.

**Acceptance criterion:**
```bash
cargo test -p anvilml-worker --test keepalive_tests
# -> >=4 tests, exits 0
```

---

### Group D — Respawn

#### P8-D1: anvilml-worker: respawn.rs RespawnPolicy backoff + max-attempt guard

**Goal:** Implement the policy that decides whether a crashed worker should be
respawned, and when, including the safety valve that halts repeated respawn
attempts.

**Files to create or modify:**
- `crates/anvilml-worker/src/respawn.rs` — `RespawnPolicy`.

**Key implementation notes:**
- Defaults per `ANVILML_DESIGN.md §19.4`: 2000ms delay, 5 max attempts within a
  300-second trailing window.
- This is a **constant-delay** policy — exponential backoff is explicitly not in
  scope; the design doc doesn't call for it, so it must not be added speculatively.

**Acceptance criterion:**
```bash
cargo test -p anvilml-worker --test respawn_tests
# -> >=5 tests, exits 0
```

---

### Group E — Worker ownership

#### P8-E1: anvilml-worker: WorkerHandle struct (cheap, Clone-able)

**Goal:** Implement the cheap, shareable handle that lets multiple independent
consumers (a status-polling task, an API handler, the pool itself) interact with a
worker's status and request its shutdown — without ever needing `Arc`-wrapping the
worker struct itself.

**Files to create or modify:**
- `crates/anvilml-worker/src/managed.rs` — `WorkerHandle`.

**Key implementation notes:**
- **Read `ANVILML_DESIGN.md §9.1` in full before writing this struct.** It
  documents the exact prior regression this shape exists to prevent: an
  `Arc`-wrapped struct with a by-value `run(self)` method that could never actually
  be called once wrapped.
- The field shape is fixed: `worker_id: String`, `status:
  Arc<RwLock<WorkerStatus>>`, `shutdown_tx: Option<oneshot::Sender<()>>`,
  `join_handle: Arc<tokio::sync::Mutex<Option<JoinHandle<()>>>>`. Cloning shares the
  lock and sender, never the worker itself.
- This task is read-only on status — a write-side mutator and `ManagedWorker`
  itself (the type that actually owns `run()`) are both separate, later tasks.

**Acceptance criterion:**
```bash
cargo test -p anvilml-worker --test managed_tests
# -> >=4 tests, exits 0
```

#### P8-E2: anvilml-worker: WorkerHandle::set_status() mutator

**Goal:** Give `WorkerHandle` its one and only public status mutator — without
this, no later phase has any way to ever transition a worker's status after
construction, which would make `WorkerStatus::Busy`/`Idle` permanently unreachable
in practice despite both being defined enum variants since Phase 3.

**Files to create or modify:**
- `crates/anvilml-worker/src/managed.rs` — adds `set_status()`.

**Key implementation notes:**
- This is the **only** public mutator on `WorkerHandle` — every later phase that
  needs to change a worker's status (dispatch marking a worker `Busy` on
  assignment, the event loop marking it `Idle` again on completion) goes through
  this single method, never a second, parallel mutation path.
- `set_status()`'s write lock and `status()`'s read lock must be independent
  acquisitions — a concurrent reader must never block a writer indefinitely, or
  vice versa.

**Acceptance criterion:**
```bash
cargo test -p anvilml-worker --test managed_tests
# -> >=8 tests total in the file, exits 0
```

#### P8-E3: anvilml-worker: ManagedWorker::run() owns full lifecycle task

**Goal:** Implement `ManagedWorker::run()` as the single owner of a worker's entire
lifecycle task, taking `self` by value for the duration of one `async fn` —
removing the ownership conflict that made `run()` uncallable in v3.

**Files to create or modify:**
- `crates/anvilml-worker/src/managed.rs` — adds `ManagedWorker`, `run()`.

**Key implementation notes:**
- `run()` takes `self` **by value** and owns the entire lifecycle within this one
  function — there is no separate, externally-callable `shutdown(self)` method
  competing with it for ownership.
- `demux.register()` is called on entry; `demux.deregister()` is called on **every**
  exit path — graceful shutdown (triggered by `shutdown_rx`), the 60-second
  `Initializing` timeout, and crash/`Dead` — not only the graceful path. This is the
  same mandatory pairing P8-C1 established at the demux level, now exercised from
  the worker lifecycle side.
- Uses exactly the `WorkerHandle` shape P8-E1/P8-E2 already completed — no
  re-derivation.

**Acceptance criterion:**
```bash
cargo test -p anvilml-worker --test managed_tests
# -> >=13 tests total in the file, exits 0
```

---

#### P8-E4: anvilml-worker: ManagedWorker tracks crash attempt_history, consults RespawnPolicy

**Goal:** Wire the decision point `RespawnPolicy` (`P8-D1`) was built for but
that nothing in the phase's original task set ever called — confirm whether a
crashed worker should be respawned, before a later task actually does it.

**Files to create or modify:**
- `crates/anvilml-worker/src/managed.rs` — adds an `attempt_history` field and
  the `should_respawn()` call on the crash exit path.
- `crates/anvilml-worker/tests/managed_tests.rs` — adds the new coverage.

**Key implementation notes:**
- Add `attempt_history: Vec<Instant>` to `ManagedWorker`, appended with the
  current time on each crash/`Dead` transition specifically — not on graceful
  shutdown and not on the 60-second `Initializing` timeout, both of which
  `P8-E3` already exits `run()` for permanently and unconditionally.
- On crash, call `self.respawn_policy.should_respawn(&self.attempt_history)`
  and log the returned boolean at `INFO`. This task only wires the decision
  point — acting on a `true` result (sleeping, re-spawning, looping) is
  `P8-E6`'s scope, deferred here. *(Citation correction: this originally read
  "P8-E5's scope" — see `PHASES.md`'s amendments log.)*

**Acceptance criterion:**
```bash
cargo test -p anvilml-worker --test managed_tests
# -> >=16 tests total in the file, exits 0
```

---

#### P8-E5: anvilml-worker: wire KeepaliveWatchdog as second crash source

**Goal:** Close the gap `P8-C2`'s own source has flagged since it was built — a
`TODO(P8-E3)` marker that nothing ever picked up. `ManagedWorker`'s only crash
signal today is a `transport.recv()` error, which never fires for a worker that
hangs or dies silently without sending anything — the watchdog is the only
mechanism that can detect that case, and it is currently dead code.

**Files to create or modify:**
- `crates/anvilml-worker/src/managed.rs` — constructs and consumes a
  `KeepaliveWatchdog` per generation.
- `crates/anvilml-worker/tests/managed_tests.rs` — adds the new coverage.

**Key implementation notes:**
- In `run()`'s existing single-generation loop (**not** an outer respawn loop —
  that refactor is `P8-E6`, immediately after this task), construct a
  `KeepaliveWatchdog` at the top of the generation: wrap `Arc::clone(&self.transport)`
  in the existing `RouterTransportAdapter` (remove its `#[allow(dead_code)]` per
  its own comment), with injectable `ping_interval`/`pong_timeout` (production
  defaults 30s/10s per `§9.2`), and spawn `watchdog.run()`.
- Add a third `tokio::select!` branch on `dead_rx`, handled **identically** to the
  existing transport-recv-error branch: append `attempt_history`, call
  `should_respawn()`, log `crash_respawn_decision`, break.
- In `handle_event()`'s `WorkerEvent::Pong` arm (currently a no-op per `P8-E3`'s
  report — confirm and fix if so), forward the event to the watchdog's `pong_tx`
  (best-effort `try_send`; a closed/full channel is not itself an error).
- Do not touch `spawn.rs` or introduce any respawn loop here — that is `P8-E6`.

**Acceptance criterion:**
```bash
cargo test -p anvilml-worker --test managed_tests
# -> >=4 new tests, exits 0
```

#### P8-E6: anvilml-worker: wire WorkerSpawner into ManagedWorker as the respawn loop

**Goal:** Complete `ANVILML_DESIGN.md §9.2`/§9.5's respawn loop — actually restart
a crashed worker's subprocess and resume `run()`'s lifecycle — using `P8-B4`'s
`WorkerSpawner` abstraction as the single, uniform spawn path for both the first
generation and every respawn.

**Files to create or modify:**
- `crates/anvilml-worker/src/managed.rs` — adds `venv_path`/`env`/`spawner` fields,
  refactors `run()` into an outer respawn loop.
- `crates/anvilml-worker/tests/managed_tests.rs` — adds the new coverage, including
  a `MockWorkerSpawner` defined directly in this test file (test-crate only; no
  shared fixture module is needed since both this task's and `P8-E7`'s tests live
  in the same file).

**Key implementation notes:**
- Add `venv_path: PathBuf`, `env: HashMap<String, String>` (built once by the
  caller via `WorkerEnv::build()`, static across respawns — none of its values
  change between generations for the same worker), and `spawner: Arc<dyn
  WorkerSpawner>` (`P8-B4`) to `ManagedWorker`.
- Refactor `run()` into an outer loop calling `self.spawner.spawn(&self.venv_path,
  self.env.clone())` at the top of **every** generation — gen 0 and every respawn,
  one uniform code path. Store the returned `Child` for the generation's lifetime;
  it must never be dropped and orphaned.
- Inner `run_once()` returns `(Self, RunOutcome)`; `RunOutcome` is `pub(crate)` —
  nothing outside the crate needs it.
- On crash (from either `P8-E5`'s watchdog or a transport error):
  `should_respawn()` `false` → exit exactly as `P8-E3` always did; `true` → status
  → `Respawning`, sleep `next_delay()`, spawn again, status → `Initializing`,
  **and call `demux.register()` again** — a confirmed missed acceptance criterion
  in the reset attempt. Test this directly via `Demux::registered()`, not
  indirectly through message flow.
- `shutdown_rx` **stays** `oneshot::Receiver<()>` — do not change its type. If a
  real defect requires reusing it across loop iterations in a way `oneshot` can't
  support, write it up under `## Blockers`/`## Deviations from Plan` for review
  rather than swapping types silently (the reset attempt swapped it to
  `watch::Receiver<bool>` while chasing an unrelated bug and never established the
  swap was actually necessary).
- `anvilml-ipc`/`transport.rs` is **out of scope** for this task entirely. Do not
  rely on `RouterTransport::close()` more than once per test, or across multiple
  respawn generations within one test — it is a one-shot, permanent transport
  shutdown by design, not a per-connection action, and this is exactly what caused
  the reset attempt's ~90-minute detour into rewriting a completed Phase 7 file.
  Simulate a second/third crash within one test via `P8-E5`'s watchdog path
  (simply withhold a pong for that generation) or a fresh malformed payload per
  generation instead. If a genuine defect in `RouterTransport` is found anyway,
  write `## Blockers`, set `Status=BLOCKED`, and STOP — do not edit it.
- Windows Job Object reassignment on respawn is `P8-E7`'s scope, deferred here.
- No `#[ignore]` anywhere, for any reason (`ANVILML_DESIGN.md §17.4` rule 5,
  `ENVIRONMENT.md`): a test that cannot pass is fixed or deleted.

**Acceptance criterion:**
```bash
cargo test -p anvilml-worker --test managed_tests
# -> >=6 new tests, exits 0, no #[ignore] anywhere
```

#### P8-E7: anvilml-worker: Windows Job Object reassignment on every respawn generation

**Goal:** Extend `JobObjectGuard` (`P8-B3`) to cover respawn — a scenario `P8-B3`'s
own acceptance criteria never exercised, since `P8-B3` predates any respawn loop.

**Files to create or modify:**
- `crates/anvilml-worker/src/managed.rs` — constructs one `JobObjectGuard` per
  `ManagedWorker`, `#[cfg(windows)]`.
- `crates/anvilml-worker/tests/managed_tests.rs` — adds the new coverage,
  `#[cfg(windows)]`-gated.

**Key implementation notes:**
- Construct **one** `JobObjectGuard` in `ManagedWorker::new()` and call
  `guard.assign_process(&child)` after every spawn in `P8-E6`'s outer loop — gen 0
  and every respawn alike, reassigning a fresh `Child` to the **same** guard each
  time.
- This is a distinct scenario from `P8-B3`'s own double-assignment test (which
  covers assigning the *same* child twice, not assigning a *different* child after
  the first has exited) — do not assume `P8-B3`'s existing coverage already proves
  this works.
- On non-Windows targets this field/step is entirely absent via `#[cfg(windows)]`
  — no behavioral change for non-Windows builds.

**Acceptance criterion:**
```bash
cargo test -p anvilml-worker --test managed_tests
# -> >=2 new tests, gated #[cfg(windows)], exits 0 on a Windows runner
cargo check -p anvilml-worker --target x86_64-pc-windows-gnu
# -> confirms the cfg-gated code compiles from the Linux agent
```

---

### Group F — Bridge

#### P8-F1: anvilml-worker: bridge.rs independent reader/writer tasks

**Goal:** Implement the two tokio tasks that actually move messages between the
worker pool's internal channels and the transport, each respecting the transport's
already-split locks without introducing any new combined lock — and establish the
**sole** production caller of `transport.recv()`, the prerequisite for `P8-F2`'s
fix of the multi-worker race that direct per-`ManagedWorker` `recv()` calls would
otherwise cause once `WorkerPool` spawns more than one worker.

**Files to create or modify:**
- `crates/anvilml-worker/src/bridge.rs` — `spawn_bridge()`.

**Key implementation notes:**
- The writer task drains an `mpsc::Receiver<WorkerMessage>` and calls
  `transport.send()`; the reader task loops `transport.recv()` and routes through
  `Demux::route()`. Each touches only its own half of the transport's split locks
  (Phase 7's P7-B2) — bridge.rs adds no lock of its own around either direction.
- Both tasks are spawned together by one function, returning the writer's input
  channel and both join handles.
- Do **not** touch `managed.rs` in this task — retrofitting `ManagedWorker` to
  actually consume from this new path is `P8-F2`, immediately after.

**Acceptance criterion:**
```bash
cargo test -p anvilml-worker --test bridge_tests
# -> >=4 tests, exits 0
```

#### P8-F2: anvilml-worker: ManagedWorker consumes its own demux channel

**Goal:** Close the multi-worker race `P8-E3` left open: `ManagedWorker::run()`
calls `self.transport.recv()` directly, which is correct for exactly one worker but
races once `WorkerPool` (`P8-G1`) spawns multiple `ManagedWorker` tasks against one
shared `Arc<RouterTransport>` — whichever task's `recv().await` wins a given poll
can consume a message addressed to a different worker.

**Files to create or modify:**
- `crates/anvilml-worker/src/managed.rs` — replaces the direct `transport.recv()`
  call with consumption from a demux-registered channel.
- `crates/anvilml-worker/tests/managed_tests.rs` — updates existing tests that
  drive events directly via `transport.recv()`.

**Key implementation notes:**
- Register an `mpsc::Sender<WorkerEvent>` with `Demux` at the top of each
  generation (paired with the existing `register()`/`deregister()` calls from
  `P8-E6`) and consume from the paired `mpsc::Receiver<WorkerEvent>` in place of
  the `transport.recv()` select branch.
- Keep the `Arc<RouterTransport>` field on `ManagedWorker` — `P8-E5`'s
  `KeepaliveWatchdog` still needs it for sending Pings.
- Update existing tests that drive events directly via `transport.recv()` to
  instead spin up `bridge::spawn_bridge()` (`P8-F1`) and drive events through a
  connected DEALER, matching production wiring exactly — this is a legitimate
  update to existing tests since this task modifies `managed.rs` itself.

**Acceptance criterion:**
```bash
cargo test -p anvilml-worker --test managed_tests
cargo test -p anvilml-worker --test bridge_tests
# -> both exit 0; existing managed_tests.rs coverage is preserved;
# >=1 new test proves two ManagedWorker instances sharing one transport/demux
# never receive each other's events
```

---

### Group G — Pool

#### P8-G1: anvilml-worker: WorkerPool spawn_all()/shutdown_all()

**Goal:** Implement `WorkerPool`, the top-level type that ties every prior task in
this phase together into the one object the scheduler phase will actually hold.

**Files to create or modify:**
- `crates/anvilml-worker/src/pool.rs` — `WorkerPool`.

**Key implementation notes:**
- `spawn_all()` calls `bridge::spawn_bridge()` (`P8-F1`) **once** for the whole
  pool against the shared transport, retaining its writer `mpsc::Sender<WorkerMessage>`
  and both `JoinHandle`s. Per device: builds `WorkerEnv`/venv_path (`P8-B1`),
  constructs a `ManagedWorker` — passing `ProcessWorkerSpawner` (`P8-B4`), the
  venv_path/env, and the shared transport/demux/respawn_policy/status — and spawns
  its `run()` task; registers the resulting `WorkerHandle`.
- `spawn_all()` itself does **not** call `spawn_worker()` or construct a
  `JobObjectGuard` directly — `ManagedWorker` (`P8-E6`/`P8-E7`) owns spawning
  uniformly across gen 0 and every respawn now; this is a deliberate change from
  the phase's original design, where the pool spawned the subprocess externally
  before constructing `ManagedWorker`.
- `shutdown_all()` requests shutdown on every handle, awaits all join handles within
  a bounded timeout (default 30s per `ANVILML_DESIGN.md §19.3` step 3), force-kills
  anything still running past that timeout (step 4), then aborts the bridge's two
  join handles as the final step.

**Acceptance criterion:**
```bash
cargo test -p anvilml-worker --features mock-hardware --test pool_tests
# -> >=5 tests, exits 0
```

---

### Group H — Closeout

#### P8-H1: anvilml-worker: lib.rs re-export pass, 80-line check

**Goal:** Finalize `anvilml-worker`'s public surface and confirm `lib.rs` stays
within the 80-line hard cap.

**Files to create or modify:**
- `crates/anvilml-worker/src/lib.rs` — re-exports only.

**Key implementation notes:**
- Confirm the Windows-only `job_object` module is correctly `cfg`-gated at its
  `mod` statement, consistent with the pattern established for `anvilml-hardware`'s
  platform-specific detectors in Phase 4.
- Confirm `WorkerSpawner`/`ProcessWorkerSpawner` (`P8-B4`) are re-exported if
  intended for external construction (`P8-G1`'s own construction site decides
  this) — verify consistency, do not silently widen or narrow visibility here.

**Acceptance criterion:**
```bash
wc -l crates/anvilml-worker/src/lib.rs
# -> <=80
cargo test -p anvilml-worker --features mock-hardware
# -> exits 0, full crate suite
```

---

## Phase Acceptance Criteria

```bash
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test --workspace --features mock-hardware
cargo test -p anvilml-ipc --test stress_test --release

# Platform cross-check (local WSL2 gate, per ENVIRONMENT.md §7):
cargo check --workspace --features mock-hardware
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu

# Runnable Proof: not applicable — this phase completes the worker supervision
# layer (spawn, supervise, respawn, demux, keepalive) but no Python worker_main.py
# exists yet for it to actually spawn and communicate with in a real subprocess —
# that integration is Phase 9's explicit scope. This phase's tests exercise the
# Rust-side machinery against mock IPC backends and simulated subprocess exits,
# which is the complete and sufficient proof of this phase's own deliverable, per
# the narrow exemption in FORGE_TASK_AUTHORING_SPEC.md §9. The IPC stress test
# (P8-A1) is itself a Runnable-Proof-grade demonstration of the transport's real
# behavior under load, and is called out explicitly above rather than only listed
# among the standard gates, per FORGE_TASK_AUTHORING_SPEC.md §9a's guidance that a
# non-standard test invocation genuinely demonstrating external behavior belongs in
# this section.
```

---

## Known Constraints and Gotchas

- The stress test (P8-A1) is not merely "another test" — it is the named gate for
  this entire phase and every later one. A regression here blocks all downstream
  work, by design.
- `WorkerHandle`/`ManagedWorker`'s ownership split (P8-E1, P8-E2, P8-E3) and `Demux`'s
  register/deregister pairing (P8-C1) are both specified exactly in the design
  document, each accompanied by the specific historical incident that produced the
  rule. Treat both as fixed contracts, not starting points for "improvement."
- `demux.deregister()` must be called from **every** exit path in
  `ManagedWorker::run()` — graceful shutdown, the `Initializing` timeout, and crash —
  not only the graceful one. A deregistration call present on only one path is the
  exact defect class this phase exists to prevent. As of `P8-E6`, `demux.register()`
  must equally be called on **every** respawn, not only the first spawn.
  `Demux::registered()` exists specifically to let tests verify this directly.
- `RespawnPolicy` is constant-delay by design; do not add exponential backoff
  without an explicit design-doc change authorizing it.
- This phase's worker subprocesses don't run any real Python code yet — `spawn_worker()`
  targets `worker/worker_main.py`, a file that doesn't exist until Phase 9. Tests in
  this phase use mock IPC backends and simulated process exits, not a real
  subprocess round trip.
- `P8-E4` was appended to this phase's original task set by an audit that traced
  `RespawnPolicy` (`P8-D1`) forward and found nothing called it. The original
  `P8-E5` — the task that was meant to close the remaining half of that gap — was
  attempted, failed in ACT, and was reset before merge (see the Overview above and
  `PHASES.md`'s amendments log for the full account). `P8-E5` through `P8-H1` in
  this document are the second-round correction: `P8-E5` now wires `P8-C2`'s
  previously-dead-code `KeepaliveWatchdog` in as a second, independent crash
  source; `P8-B4`/`P8-E6` split the respawn loop's spawn abstraction from its
  wiring per `FORGE_TASK_AUTHORING_SPEC.md §10`'s remedy for oversized tasks
  ("data structure vs behaviour lines"), tracked with an explicit `defers_to`
  rather than left as a silent gap; `P8-E7` closes a Windows-specific scenario
  `P8-B3` never had reason to test; `P8-F2` closes a multi-worker race in
  `P8-E3`'s original `transport.recv()` usage, found during the same review, that
  would otherwise have first surfaced once `P8-G1` spawned more than one worker.
- `anvilml-ipc`/`transport.rs` (Phase 7, completed) is out of scope for every task
  in this phase. A defect found there while working on any Phase 8 task is a
  blocker to write up and stop on (`FORGE_AGENT_RULES.md §9.4`), never an in-task
  fix — this is exactly what the reset `P8-E5` attempt got wrong.
- No `#[ignore]` in committed code, ever, for any reason (`ANVILML_DESIGN.md
  §17.4` rule 5, `ENVIRONMENT.md`). A test that cannot pass is fixed or deleted.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 8 — IPC Stress Gate & Worker Pool

**Capability proved:** The IPC transport survives 1000 sustained ROUTER/DEALER
round trips with zero message loss or reordering — the explicit gate named in
`ANVILML_DESIGN.md §20`'s IPC Baseline roadmap entry. The worker supervision layer
(spawn, demux, keepalive, respawn, pool) is complete and tested against mock IPC
backends, though it has no real Python subprocess to supervise yet — that
integration is Phase 9's scope.

\`\`\`bash
cargo test -p anvilml-ipc --test stress_test --release
# -> exits 0, all 1000 round trips complete with zero loss
\`\`\`
```