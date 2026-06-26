# Tasks: Phase 15 — Artifact Storage Wiring

**Phase:** 15
**Name:** Artifact Storage Wiring
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 2, 3, 6, 7, 14

---

## Overview

This phase exposes Phase 6's already-complete `ArtifactStore` over HTTP for the
first time (`GET /v1/artifacts`, `GET /v1/artifacts/:hash`), and connects the
`WorkerEvent::ImageReady` event — defined back in Phase 7 but never consumed by
anything since — to that store, via a new `event_loop.rs` module in
`anvilml-scheduler`. This is the first task in the entire project that actually
listens for `ImageReady` and does something with it.

This phase exists right after Dispatch & Execute (Phase 14) because artifact
persistence only matters once jobs can actually dispatch and run — Phase 14 proved
the pipeline works end-to-end with `PassThrough`, a node that produces no image
output at all, so artifact wiring genuinely had nothing to consume until now. This
phase closes that gap structurally (the event consumer and HTTP surface both exist
and are tested) even though, in this delivery, no real image-producing node exists
yet to exercise it with actual image data — that arrives with the later
architecture-loading phases (`LoadModel` → `Sampler` → `VaeDecode` → `SaveImage`).

At the start of this phase, `ArtifactStore` is fully implemented (Phase 6) but
unreachable from outside the test suite, and `ImageReady` is a defined-but-unused
enum variant. At the end: `GET /v1/artifacts*` is live and tested, and
`anvilml-scheduler`'s new `event_loop.rs` saves any `ImageReady` payload it receives
— though this phase's own Runnable Proof can only demonstrate the endpoint
correctly returning an empty list, since `PassThrough` is still the only node in the
project and it never emits `ImageReady`.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Server state | P15-A1 | `AppState` gains `artifact_store` |
| B | HTTP handlers | P15-B1 … P15-B2 | `GET /v1/artifacts`, then `GET /v1/artifacts/:hash` |
| C | Event consumption | P15-C1 | `event_loop.rs` saves `ImageReady` payloads |
| D | Proof | P15-D1 | The phase's Runnable Proof |

---

## Prerequisites

`ArtifactStore` must be complete per Phase 6 (P6-B1, P6-B2, P6-B3). `AppState` must
already hold `scheduler`/`workers`/`db` per Phase 14 (P14-C1). `WorkerEvent::ImageReady`
must exist exactly as defined in Phase 7 (P7-A4).

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §13.4` | P15-B1, P15-B2 | Exact route shapes for `/v1/artifacts` and `/v1/artifacts/:hash` |
| `ANVILML_DESIGN.md §3.3` | P15-B1 | "No business logic in handler functions" |
| `ANVILML_DESIGN.md §12.1` | P15-C1 | `event_loop.rs`'s module placement in `anvilml-scheduler` |
| `ANVILML_DESIGN.md §8.6` | P15-C1 | `WorkerEvent::ImageReady`'s exact field shape |

---

## Task Descriptions

### Group A — Server state

#### P15-A1: anvilml-server: AppState gains artifact_store field

**Goal:** Grow `AppState` with the one field this phase's handlers need,
continuing the established incremental-growth pattern.

**Files to create or modify:**
- `crates/anvilml-server/src/state.rs` — adds `artifact_store: Arc<ArtifactStore>`.
- `backend/src/main.rs` — constructs an `ArtifactStore` sharing the existing
  `SqlitePool`, passes it into `AppState`.

**Key implementation notes:**
- `artifact_dir` comes from `cfg.artifact_dir` (Phase 2's `ServerConfig`).
- The remaining `ANVILML_DESIGN.md §13.2` fields (`hardware`, `broadcaster`,
  `env_report`) stay absent — added only when a task actually needs them.

**Acceptance criterion:**
```bash
cargo test -p anvilml-server --test state_tests
cargo build -p anvilml
# -> both exit 0
```

---

### Group B — HTTP handlers

#### P15-B1: anvilml-server: GET /v1/artifacts list handler

**Goal:** Expose artifact listing over HTTP, delegating entirely to the store with
zero business logic in the handler.

**Files to create or modify:**
- `crates/anvilml-server/src/handlers/artifacts.rs` — `list_artifacts()`.

**Key implementation notes:**
- `ListArtifactsParams { job_id: Option<Uuid> }` is the query parameter struct —
  `None` returns every artifact, `Some(id)` filters.
- The handler is a one-line delegation to `state.artifact_store.list()`, per
  `ANVILML_DESIGN.md §3.3`.

**Acceptance criterion:**
```bash
cargo test -p anvilml-server --test artifacts_tests
# -> >=4 tests, exits 0
```

#### P15-B2: anvilml-server: GET /v1/artifacts/:hash serve PNG bytes

**Goal:** Expose individual artifact retrieval by content hash, serving the raw
PNG bytes with the correct content type.

**Files to create or modify:**
- `crates/anvilml-server/src/handlers/artifacts.rs` — adds `get_artifact()`.

**Key implementation notes:**
- `Content-Type: image/png` on success; `404` via `AnvilError::ArtifactNotFound(hash)`
  on an unknown hash — the dedicated variant added in Phase 2's `P2-A1` per
  `docs/ADDENDUM_ARTIFACT_NOT_FOUND.md`. The prior session's Deviation note on this
  point is now resolved — no `Internal` placeholder remains.

**Acceptance criterion:**
```bash
cargo test -p anvilml-server --test artifacts_tests
# -> >=8 tests total in the file, exits 0
```

---

### Group C — Event consumption

#### P15-C1: anvilml-scheduler: dispatch_one persists ArtifactMeta on ImageReady

**Goal:** Implement `event_loop.rs` — the module named in
`ANVILML_DESIGN.md §12.1`'s layout since the design was written, but not created by
any prior phase — and give `WorkerEvent::ImageReady` its first real consumer.

**Files to create or modify:**
- `crates/anvilml-scheduler/src/event_loop.rs` — new; handles `ImageReady`.
- `crates/anvilml-scheduler/src/scheduler.rs` — `JobScheduler` gains an
  `Arc<ArtifactStore>` constructor field.

**Key implementation notes:**
- This is the **first** task in the entire project that does anything with
  `ImageReady` — the variant has existed since Phase 7 (P7-A4) with no consumer
  until now.
- Decodes `image_b64`, calls `artifact_store.save()` with the decoded bytes and a
  constructed `ArtifactMeta`, persisting under the resulting content hash.

**Acceptance criterion:**
```bash
cargo test -p anvilml-scheduler --test event_loop_tests
# -> >=4 tests, exits 0
```

---

### Group D — Proof

#### P15-D1: Runnable Proof: PassThrough-derived job artifact retrievable via HTTP

**Goal:** Produce this phase's Runnable Proof, with an explicit, honest
acknowledgment of its scope limit: `PassThrough` produces no image output, so the
proof demonstrates the endpoint's correctness on an empty result, not a populated
one.

**Files to create or modify:**
- None. This task runs the already-built binary; see Acceptance Criterion.

**Key implementation notes:**
- `GET /v1/artifacts` correctly returning `[]` against the current
  `PassThrough`-only graph is the right and complete proof available at this point
  in the project — not an incomplete deliverable. The first job that produces a
  genuinely retrievable artifact requires a real image-producing node chain
  (`LoadModel` → `Sampler` → `VaeDecode` → `SaveImage`), which arrives with the
  later architecture-loading phases, explicitly out of this phase's scope.

**Acceptance criterion:**
```bash
cargo build --release -p anvilml --features mock-hardware
./target/release/anvilml &
sleep 1
curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8488/v1/artifacts
# -> 200
curl -s http://127.0.0.1:8488/v1/artifacts
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
curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8488/v1/artifacts
# -> 200
curl -s http://127.0.0.1:8488/v1/artifacts
# -> []
kill %1
```

---

## Known Constraints and Gotchas

- `GET /v1/artifacts` returning `[]` in this phase's Runnable Proof is correct, not
  incomplete — `PassThrough` is still the only node in the project and produces no
  image output. Don't treat the empty result as a sign anything is broken.
- `AnvilError::ArtifactNotFound(String)` (Phase 2's `P2-A1`, see
  `docs/ADDENDUM_ARTIFACT_NOT_FOUND.md`) is the dedicated 404 variant
  `get_artifact()` uses — distinct from `WorkerNotFound`/`JobNotFound`/
  `ModelNotFound`, each named for the resource it identifies. This resolves a gap
  flagged in an earlier session; no `Internal` placeholder remains.
- `event_loop.rs` is the first module in `anvilml-scheduler` that actually consumes
  `WorkerEvent`s from the demux/bridge layer — confirm it doesn't duplicate any
  event-routing logic already owned by `anvilml-worker`'s `demux.rs` (Phase 8).
- `JobScheduler` gaining an `Arc<ArtifactStore>` field is additive to its existing
  constructor signature from Phase 14 — confirm all existing call sites
  (`backend/main.rs`, test fixtures) are updated consistently.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 15 — Artifact Storage Wiring

**Capability proved:** `GET /v1/artifacts` and `GET /v1/artifacts/:hash` are live
over a real HTTP server, backed by Phase 6's `ArtifactStore`. The proof shows a
correct empty-list response, since `PassThrough` (the only node in the project at
this point) produces no image output — the first populated response requires a
real image-producing node chain, added in a later phase.

\`\`\`bash
# Runnable Proof (manual):
cargo build --release -p anvilml --features mock-hardware
./target/release/anvilml &
sleep 1
curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8488/v1/artifacts
# -> 200
curl -s http://127.0.0.1:8488/v1/artifacts
# -> []
kill %1
\`\`\`
```
