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

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Stress gate | P8-A1 | The 1000-round-trip test — gates this phase and every later one |
| B | Spawning | P8-B1 … P8-B3 | `WorkerEnv`, `spawn_worker()`, Windows Job Object orphan cleanup |
| C | Demux & keepalive | P8-C1 … P8-C2 | `Demux` with mandatory `deregister()`, the ping/pong watchdog |
| D | Respawn | P8-D1 | `RespawnPolicy` backoff and max-attempt guard |
| E | Worker ownership | P8-E1 … P8-E2 | `WorkerHandle` (cheap, `Clone`-able), then `ManagedWorker::run()` |
| F | Bridge | P8-F1 | The two independent reader/writer tasks against the split transport |
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
| `ANVILML_DESIGN.md §9.1` | P8-E1, P8-E2 | `WorkerHandle`/`ManagedWorker`'s exact ownership shape — read in full before either task |
| `ANVILML_DESIGN.md §9.6` | P8-F1 | The bridge's two-independent-tasks shape, reusing the already-split transport locks |
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
- Windows Job Object wrapping is explicitly deferred to the next task.

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
- `ManagedWorker` (the type that actually owns `run()`) is the next task's scope —
  this task is the handle alone.

**Acceptance criterion:**
```bash
cargo test -p anvilml-worker --test managed_tests
# -> >=4 tests, exits 0
```

#### P8-E2: anvilml-worker: ManagedWorker::run() owns full lifecycle task

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
- This receives exactly the construction P8-E1 already completed — use that exact
  `WorkerHandle` shape; do not re-derive it.

**Acceptance criterion:**
```bash
cargo test -p anvilml-worker --test managed_tests
# -> >=9 tests total in the file, exits 0
```

---

### Group F — Bridge

#### P8-F1: anvilml-worker: bridge.rs independent reader/writer tasks

**Goal:** Implement the two tokio tasks that actually move messages between the
worker pool's internal channels and the transport, each respecting the transport's
already-split locks without introducing any new combined lock.

**Files to create or modify:**
- `crates/anvilml-worker/src/bridge.rs` — `spawn_bridge()`.

**Key implementation notes:**
- The writer task drains an `mpsc::Receiver<WorkerMessage>` and calls
  `transport.send()`; the reader task loops `transport.recv()` and routes through
  `Demux::route()`. Each touches only its own half of the transport's split locks
  (Phase 7's P7-B2) — bridge.rs adds no lock of its own around either direction.
- Both tasks are spawned together by one function, returning the writer's input
  channel and both join handles.

**Acceptance criterion:**
```bash
cargo test -p anvilml-worker --test bridge_tests
# -> >=4 tests, exits 0
```

---

### Group G — Pool

#### P8-G1: anvilml-worker: WorkerPool spawn_all()/shutdown_all()

**Goal:** Implement `WorkerPool`, the top-level type that ties every prior task in
this phase together into the one object the scheduler phase will actually hold.

**Files to create or modify:**
- `crates/anvilml-worker/src/pool.rs` — `WorkerPool`.

**Key implementation notes:**
- `spawn_all()` composes, per device: `WorkerEnv` (P8-B1) → subprocess spawn (P8-B2,
  plus Windows Job Object via P8-B3) → a `ManagedWorker` (P8-E2) whose `run()` task
  is spawned → the resulting `WorkerHandle` registered into the pool.
- `shutdown_all()` requests shutdown on every handle, awaits all join handles within
  a bounded timeout (default 30s per `ANVILML_DESIGN.md §19.3` step 3), and
  force-kills anything still running past that timeout, per step 4.

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
- `WorkerHandle`/`ManagedWorker`'s ownership split (P8-E1, P8-E2) and `Demux`'s
  register/deregister pairing (P8-C1) are both specified exactly in the design
  document, each accompanied by the specific historical incident that produced the
  rule. Treat both as fixed contracts, not starting points for "improvement."
- `demux.deregister()` must be called from **every** exit path in
  `ManagedWorker::run()` — graceful shutdown, the `Initializing` timeout, and crash —
  not only the graceful one. A deregistration call present on only one path is the
  exact defect class this phase exists to prevent.
- `RespawnPolicy` is constant-delay by design; do not add exponential backoff
  without an explicit design-doc change authorizing it.
- This phase's worker subprocesses don't run any real Python code yet — `spawn_worker()`
  targets `worker/worker_main.py`, a file that doesn't exist until Phase 9. Tests in
  this phase use mock IPC backends and simulated process exits, not a real
  subprocess round trip.

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
