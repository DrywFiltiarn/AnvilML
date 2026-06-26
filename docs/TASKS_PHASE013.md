# Tasks: Phase 13 — Job Queue

**Phase:** 13
**Name:** Job Queue
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 2, 3, 6, 12

---

## Overview

This phase builds the in-memory queue and VRAM ledger `anvilml-scheduler`'s
dispatch loop (a later phase) will need, plus the persistence side: a `jobs` table
migration and a `JobStore` in `anvilml-registry` that mirrors `anvilml-core`'s `Job`
struct to SQLite, including the ghost-job reset `ANVILML_DESIGN.md §19.2` requires at
every server startup. No dispatch loop and no `POST /v1/jobs` handler exist yet —
this phase is the queue, the ledger, and persistence in isolation, each independently
testable before the dispatch logic that ties them together.

This phase exists right after graph validation (Phase 12) and before dispatch (a
later phase) because the dispatch loop needs a real queue and a real ledger to
operate on — building dispatch logic before either exists would mean either
inventing throwaway stand-ins or conflating two distinct concerns (queueing/ledger
bookkeeping vs. the policy that decides what to dispatch when) into one task. Job
persistence is deliberately placed in `anvilml-registry`, not `anvilml-scheduler`,
mirroring the same split Phase 6 already established between `ModelStore`
(persistence) and `ModelScanner` (logic) — the scheduler's queue is purely
in-memory; `anvilml-registry` is where anything touching SQLite belongs.

At the start of this phase, no `jobs` table exists and `anvilml-scheduler` has only
Phase 12's validator. At the end: `JobQueue` provides FIFO push/pop with O(1)
cancellation via lazy removal; `VramLedger` tracks per-device reservations
advisorily; `JobStore` persists `Job` rows and resets any ghost `Queued`/`Running`
jobs to `Failed` on startup — and `backend/main.rs`'s normal startup path now
actually calls that reset, the first time `main.rs` constructs a real `SqlitePool` in
its non-`hw-probe` run path.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | In-memory scheduler primitives | P13-A1 … P13-A3 | `jobs` table migration, `JobQueue`, `VramLedger` |
| B | Persistence | P13-B1 | `JobStore` CRUD + ghost-job reset |
| C | Startup wiring | P13-C1 | `backend/main.rs` calls `reset_ghost_jobs()` at startup |
| D | Closeout | P13-D1 | `lib.rs` re-export pass, 80-line check |

---

## Prerequisites

`anvilml-core` must export `Job`, `JobStatus`, `JobSettings` exactly as defined in
Phase 3 (P3-A1). `anvilml-registry`'s `create_pool()` and migration runner must work
per Phase 6 (P6-A2, P6-A8). `anvilml-scheduler`'s `ValidatedGraph`/`GraphError`/
`validate_graph()` must exist per Phase 12 (P12-A6).

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §12.1` | P13-A2, P13-A3, P13-D1 | `queue.rs`/`ledger.rs` module layout |
| `ANVILML_DESIGN.md §12.4` | P13-A3 | The ledger is advisory, never enforced — release on `Completed`/`Failed`/`Cancelled` |
| `ANVILML_DESIGN.md §19.2` | P13-B1, P13-C1 | Exact ghost-job reset behavior and error string |
| `ANVILML_DESIGN.md §3.3` | P13-B1 | Job persistence belongs to `anvilml-registry`, mirroring the `ModelStore`/`ModelScanner` split |

---

## Task Descriptions

### Group A — In-memory scheduler primitives

#### P13-A1: database/: jobs table migration

**Goal:** Create the persistence schema for jobs before any store code needs it.

**Files to create or modify:**
- `database/migrations/003_jobs.sql` — `jobs` table + two indexes.

**Key implementation notes:**
- `graph` and `settings` are stored as `TEXT` (serialized JSON), not normalized
  columns — `anvilml-core`'s `Job` struct already owns the canonical in-memory
  shape; this table is purely a persistence mirror of it, not a second source of
  truth for its structure.

**Acceptance criterion:**
```bash
sqlite3 :memory: < database/migrations/003_jobs.sql
# -> exit 0
```

#### P13-A2: anvilml-scheduler: JobQueue in-memory FIFO with O(1) cancel

**Goal:** Implement the in-memory queue the dispatch loop will pop jobs from,
with a cancellation mechanism that doesn't require an O(n) scan of the underlying
deque.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/queue.rs` — `JobQueue`.

**Key implementation notes:**
- Cancellation is **lazy**: `cancel()` only marks an ID in a side `HashSet`; it
  does not search and remove from the `VecDeque` immediately. `pop_front()` is what
  actually discards a cancelled entry when it's encountered — this is the mechanism
  that makes cancellation O(1) rather than O(n).
- Pure in-memory, no async, no I/O — persistence is P13-B1's separate concern.

**Acceptance criterion:**
```bash
cargo test -p anvilml-scheduler --test queue_tests
# -> >=7 tests, exits 0
```

#### P13-A3: anvilml-scheduler: VramLedger per-device reservation tracking

**Goal:** Implement the advisory VRAM accounting the dispatch loop will consult
before assigning a job to a worker, without ever claiming to prevent an actual OOM.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/ledger.rs` — `VramLedger`.

**Key implementation notes:**
- **Advisory, not enforced** — per `ANVILML_DESIGN.md §12.4` exactly: the ledger
  prevents over-scheduling, not VRAM sufficiency. A real OOM remains possible; the
  worker emits `Failed` and the scheduler calls `release()` regardless.
- `release()` uses saturating subtraction — releasing more than was reserved (which
  can legitimately happen with imprecise estimates) must never panic or underflow.

**Acceptance criterion:**
```bash
cargo test -p anvilml-scheduler --test ledger_tests
# -> >=6 tests, exits 0
```

---

### Group B — Persistence

#### P13-B1: anvilml-registry: JobStore CRUD, ghost-job reset on startup

**Goal:** Implement job persistence and the ghost-job reset that must run at every
server startup, placed in `anvilml-registry` rather than `anvilml-scheduler` per the
project's established persistence-vs-logic crate split.

**Files to create or modify:**
- `crates/anvilml-registry/src/job_store.rs` — `JobStore`.

**Key implementation notes:**
- This lives in **`anvilml-registry`**, not `anvilml-scheduler` — job persistence is
  a registry concern, mirroring the exact split Phase 6 established between
  `ModelStore` (persistence) and `ModelScanner` (logic).
- `reset_ghost_jobs()` implements `ANVILML_DESIGN.md §19.2` precisely: any job in
  `Queued` or `Running` state is reset to `Failed` with `error: "server_restart"` —
  the literal string matters, since operators and later diagnostics key off it.

**Acceptance criterion:**
```bash
cargo test -p anvilml-registry --test job_store_tests
# -> >=6 tests, exits 0
```

---

### Group C — Startup wiring

#### P13-C1: backend: wire reset_ghost_jobs() into server startup sequence

**Goal:** Connect the binary's normal startup path to the ghost-job reset, the
first time `main.rs` constructs a real `SqlitePool` outside the `hw-probe`
subcommand.

**Files to create or modify:**
- `backend/src/main.rs` — constructs a pool, a `JobStore`, calls
  `reset_ghost_jobs()`, logs the affected count at `INFO` if nonzero.

**Key implementation notes:**
- This is the first task where `main.rs`'s normal (non-`hw-probe`) run path
  actually creates a `SqlitePool` — Phase 6 only built the capability;
  `AppState` doesn't yet hold a `db` field, and that field is deliberately not
  added here speculatively — it arrives when a later task actually needs `AppState`
  to hold the pool.

**Acceptance criterion:**
```bash
cargo build -p anvilml
cargo test --workspace --features mock-hardware
# -> both exit 0
```

---

### Group D — Closeout

#### P13-D1: anvilml-scheduler: lib.rs re-export pass, 80-line check

**Goal:** Finalize `anvilml-scheduler`'s public surface for this phase's additions
and confirm `lib.rs` stays within the 80-line hard cap.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/lib.rs` — re-exports only.

**Key implementation notes:**
- Confirms Phase 12's `ValidatedGraph`/`GraphError`/`validate_graph` re-exports are
  still present alongside this phase's new `JobQueue`/`VramLedger` re-exports.

**Acceptance criterion:**
```bash
wc -l crates/anvilml-scheduler/src/lib.rs
# -> <=80
cargo test -p anvilml-scheduler
# -> exits 0, full crate suite
```

---

## Phase Acceptance Criteria

```bash
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test --workspace --features mock-hardware

# Runnable Proof: not applicable — this phase implements in-memory queue/ledger
# primitives and a persistence store with no HTTP handler, CLI subcommand, or
# other externally observable surface beyond the ghost-job reset's INFO log line
# at startup (which is observable but not a distinct, scriptable assertion target
# the way an HTTP response is). The full test suite (queue_tests, ledger_tests,
# job_store_tests) is the complete and sufficient proof of this phase's
# deliverable, per the narrow exemption in FORGE_TASK_AUTHORING_SPEC.md §9.
# POST /v1/jobs and the dispatch loop (a later phase) are the eventual real
# consumers of everything built here.
```

---

## Known Constraints and Gotchas

- `JobQueue::cancel()` is O(1) by design — it marks, it does not remove. Any future
  change that makes `cancel()` scan and remove from the `VecDeque` directly would
  reintroduce the O(n) cost this design specifically avoids.
- `VramLedger` is advisory — no code anywhere in this phase or later should treat a
  successful `reserve()` as a guarantee against OOM. The worker's own `Failed` event
  is still the authoritative signal that something went wrong.
- `JobStore` lives in `anvilml-registry`, not `anvilml-scheduler` — this placement is
  deliberate and mirrors an existing pattern (`ModelStore` vs. `ModelScanner`), not
  an inconsistency to "fix" by moving it later.
- `AppState` does not gain a `db` field in this phase — `backend/main.rs` constructs
  its own pool locally for the ghost-job reset, since `AppState` doesn't need to hold
  it yet for anything this phase builds.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 13 — Job Queue

**Capability proved:** Not applicable — this phase implements in-memory queue/
ledger primitives and job persistence with no HTTP handler wired up yet. The
ghost-job reset now runs at every server startup, observable only via its INFO log
line. See `TASKS_PHASE013.md`'s Phase Acceptance Criteria for the full test-suite
proof. `POST /v1/jobs` and the dispatch loop (later phases) are the eventual real
consumers.
```
