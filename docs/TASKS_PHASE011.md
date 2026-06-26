# Tasks: Phase 11 — Dynamic Node System

**Phase:** 11
**Name:** Dynamic Node System
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 2, 3, 8, 9, 10

---

## Overview

This phase closes the loop Phase 10 deliberately left open: a worker's `Ready` event
now actually populates `anvilml-core`'s `NodeTypeRegistry`, and that registry is
exposed for the first time through a real HTTP endpoint, `GET /v1/nodes`. This is
also the first phase that creates `anvilml-server`'s `AppState` and wires
`build_router()` to actually use it — the first piece of the eventual full server,
kept deliberately minimal (just the two fields this phase's one handler needs)
rather than front-loading every field `ANVILML_DESIGN.md §13.2` eventually specifies.

This phase exists right after node groundwork (Phase 10) and before graph validation
(Phase 12) because graph validation needs a real, queryable node registry to check
node types against — building the validator before this phase would mean validating
against a permanently-empty registry with no way to ever populate it. This phase is
also explicitly named in `ANVILML_DESIGN.md §20`'s roadmap as the phase where "worker
reports node types at Ready; Rust stores them in dynamic registry; `/v1/nodes`
endpoint live" — distinct from Phase 10's groundwork and from the later phases that
actually add concrete node types.

At the start of this phase, `NodeTypeRegistry` exists (Phase 3) but nothing ever
calls `register_all()` on it, and `anvilml-server` has no `AppState` or handlers at
all beyond Phase 1's bare `/health`. At the end: `ManagedWorker::run()` populates the
registry on every `Ready` event (including on respawn), and a live server answers
`GET /v1/nodes` with whatever the registry currently holds — which is still an empty
array at this point in the project, since no worker is spawned by `backend/main.rs`'s
normal server-start path yet (that integration is a later phase).

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Worker-side wiring | P11-A1 | `ManagedWorker` calls `register_all()` on every `Ready` event |
| B | Server state | P11-B1 | `AppState` (minimal — `config`, `node_registry` only) |
| C | Handler | P11-C1 | `GET /v1/nodes` |
| D | Binary wiring | P11-D1 | `backend/main.rs` constructs `AppState` and uses `build_router()` |
| E | Proof | P11-E1 | The phase's Runnable Proof |

---

## Prerequisites

`NodeTypeRegistry` must exist exactly as defined in Phase 3 (P3-A10).
`ManagedWorker::run()` must exist and pass its own tests per Phase 8 (P8-E2), and
`worker_main.py`'s real and mock `Ready` events must both carry a (currently empty)
`node_types` list per Phase 9/10 (P9-D2, P9-D3, P10-D1).

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §10.2` | P11-A1 | "No compile-time node type list" — the registry is populated **only** from worker `Ready` events |
| `ANVILML_DESIGN.md §13.2` | P11-B1 | `AppState`'s eventual full field list — this phase populates only the subset it needs |
| `ANVILML_DESIGN.md §13.4` | P11-C1 | `GET /v1/nodes`'s exact route and response shape |
| `ANVILML_DESIGN.md §3.3` | P11-C1 | "No business logic in handler functions" — handlers delegate, they don't implement |

---

## Task Descriptions

### Group A — Worker-side wiring

#### P11-A1: anvilml-worker: ManagedWorker calls node_registry.register_all() on Ready

**Goal:** Connect the worker lifecycle to the dynamic node registry — the single
call site that ever populates it, closing the gap Phase 10 left open by design.

**Files to create or modify:**
- `crates/anvilml-worker/src/managed.rs` — `ManagedWorker::run()` gains an
  `Arc<NodeTypeRegistry>` constructor parameter.

**Key implementation notes:**
- On receiving `WorkerEvent::Ready`, `register_all(event.node_types)` is called
  **before** the worker's status transitions to `Idle` — node types are available
  the moment the worker becomes usable, not after.
- On respawn, the worker re-reports its node types and `register_all()` is called
  again, **replacing** the registry's contents per its own documented semantics
  (Phase 3's P3-A10) — this is correct even when the respawned worker reports the
  exact same node types as before.

**Acceptance criterion:**
```bash
cargo test -p anvilml-worker --test managed_tests
# -> >=4 new tests, exits 0
```

---

### Group B — Server state

#### P11-B1: anvilml-server: AppState struct (initial fields only)

**Goal:** Create `AppState` with exactly the two fields this phase's handler needs,
establishing the pattern of growing this struct incrementally rather than
front-loading fields with no consumer yet.

**Files to create or modify:**
- `crates/anvilml-server/src/state.rs` — `AppState { config, node_registry }`.

**Key implementation notes:**
- `ANVILML_DESIGN.md §13.2` eventually specifies ten fields on `AppState`
  (`scheduler`, `workers`, `registry`, `hardware`, `db`, `broadcaster`,
  `artifact_store`, `env_report`, plus this phase's two) — adding the other eight
  now, with nothing yet to construct or consume them, would fail clippy's dead-code
  lint and contradicts the project's no-speculative-scope convention.
- `#[derive(Clone)]` — cloning `AppState` shares the same `Arc`-wrapped
  `node_registry`, never a separate copy.

**Acceptance criterion:**
```bash
cargo test -p anvilml-server --test state_tests
# -> >=2 tests, exits 0
```

---

### Group C — Handler

#### P11-C1: anvilml-server: GET /v1/nodes handler

**Goal:** Implement the first real HTTP handler beyond `/health`, exposing the
node registry's contents over the network for the first time.

**Files to create or modify:**
- `crates/anvilml-server/src/handlers/nodes.rs` — `list_nodes()`.
- `crates/anvilml-server/src/handlers/mod.rs` — adds `mod nodes;`.
- `crates/anvilml-server/src/lib.rs` — registers the route, wires `.with_state()`.

**Key implementation notes:**
- The handler is a one-line delegation to `state.node_registry.list()` — per
  `ANVILML_DESIGN.md §3.3`'s hard constraint that handlers contain no business
  logic, this is exactly the correct shape, not an oversimplification to expand
  later.
- This is the first task in `anvilml-server` to actually call `.with_state()` on
  the router — confirm the state extractor wiring compiles correctly, since every
  later handler in later phases follows this same pattern.

**Acceptance criterion:**
```bash
cargo test -p anvilml-server --test nodes_tests
# -> >=4 tests, exits 0
```

---

### Group D — Binary wiring

#### P11-D1: backend: wire AppState construction + build_router into main.rs

**Goal:** Connect the binary's normal server-start path to the now-real
`AppState`/`build_router()`, replacing the bare router Phase 1 originally wired up.

**Files to create or modify:**
- `backend/src/main.rs` — constructs `NodeTypeRegistry::new()` and `AppState`,
  passes it to `build_router()`.

**Key implementation notes:**
- This is tagged `breaking` because it changes how `main.rs` constructs its router
  — confirm the existing `/health` route (Phase 1) and the `hw-probe` subcommand
  path (Phase 5) both still work unmodified.
- The registry is constructed empty and **stays** empty through this phase — no
  worker is spawned by the normal server-start path yet; that integration belongs
  to a later phase that actually wires `WorkerPool` into `main.rs`.

**Acceptance criterion:**
```bash
cargo build -p anvilml
cargo test --workspace --features mock-hardware
# -> both exit 0
```

---

### Group E — Proof

#### P11-E1: Runnable Proof: live binary serves GET /v1/nodes with real data

**Goal:** Produce this phase's Runnable Proof — confirming the built binary
actually answers `GET /v1/nodes` over a real HTTP request — and record the
transcript.

**Files to create or modify:**
- None. This task runs the already-built binary; see Acceptance Criterion.

**Key implementation notes:**
- The response is `[]` at this point in the project, and that's the correct,
  expected result — not a sign anything is missing. The first phase where this
  response becomes non-empty is the later phase that spawns a real `WorkerPool`
  from the normal server-start path.

**Acceptance criterion:**
```bash
cargo build --release -p anvilml --features mock-hardware
./target/release/anvilml &
sleep 1
curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8488/v1/nodes
# -> 200
curl -s http://127.0.0.1:8488/v1/nodes
# -> []
kill %1
```

---

## Phase Acceptance Criteria

```bash
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test --workspace --features mock-hardware

# Runnable Proof (manual):
cargo build --release -p anvilml --features mock-hardware
./target/release/anvilml &
sleep 1
curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8488/v1/nodes
# -> 200
curl -s http://127.0.0.1:8488/v1/nodes
# -> []
kill %1
```

---

## Known Constraints and Gotchas

- `AppState` must not gain fields speculatively in this phase — only `config` and
  `node_registry` are needed by this phase's one handler. Every later field is
  added by the task that actually needs it.
- `register_all()` is called from exactly one place in the entire codebase:
  `ManagedWorker::run()`'s `Ready`-event handling. No other code path may call it.
- `GET /v1/nodes` returning `[]` in this phase's Runnable Proof is correct — it
  proves the wiring works, not that nodes exist yet. Don't mistake this for an
  incomplete deliverable.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 11 — Dynamic Node System

**Capability proved:** The running `anvilml` binary serves `GET /v1/nodes` over a
real HTTP request, backed by the dynamic `NodeTypeRegistry` that `ManagedWorker`
populates on every `Ready` event — though the registry is still empty at this point
in the project, since no worker is spawned by the normal server-start path yet.

\`\`\`bash
# Runnable Proof (manual):
cargo build --release -p anvilml --features mock-hardware
./target/release/anvilml &
sleep 1
curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8488/v1/nodes
# -> 200
curl -s http://127.0.0.1:8488/v1/nodes
# -> []
kill %1
\`\`\`
```
