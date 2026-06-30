# Tasks: Phase 18 — HTTP/WebSocket Server Completion

**Phase:** 18
**Name:** HTTP/WebSocket Server Completion
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 2, 3, 5, 6, 8, 14, 16, 17

---

## Overview

This phase fills in every remaining handler from `ANVILML_DESIGN.md §13.4`'s REST
route table: `/v1/system*`, `/v1/models*`, `/v1/workers*`, and `DELETE /v1/jobs*`.
`AppState` gains its final two fields (`hardware`, `env_report`), completing the
full ten-field struct `§13.2` specifies. This is also the phase where
`anvilml-openapi` goes from Phase 1's stub to a real generator — the first time
`api/openapi.json` has actual content, and the first time the `openapi-drift` CI
gate (a placeholder since Phase 1) does anything meaningful.

This phase exists at this point because every remaining route depends on
infrastructure finished in earlier phases — `ModelStore` (Phase 6), `WorkerPool`
(Phase 8), the terminal-job-status persistence and artifact deletion path (Phases
13–16) — but none of those capabilities had an HTTP surface yet. Closing out the
REST API here, after cancellation (Phase 17) rather than before it, follows the
same incremental-`AppState`-growth discipline every server phase since Phase 11 has
used: build the field, build the handler that needs it, never both speculatively
ahead of demand.

At the start of this phase, `AppState` is missing `hardware`/`env_report`/
`model_store`, and `/v1/system*`, `/v1/models*`, `/v1/workers*`, and
`DELETE /v1/jobs*` don't exist. At the end: every route in `§13.4`'s table is live
and backed by real logic, `api/openapi.json` is generated from real `utoipa`
annotations on every handler, and the `openapi-drift` CI gate actually checks
something.

`P18-C3`, appended to this phase by a later audit, closes a gap in the model
registry's reachability that the phase's original `P18-C1`/`P18-C2` left open:
`ModelScanner::scan_dir()` (Phase 6) was only ever invoked by `P18-C2`'s
`POST /v1/models/rescan` handler — no task triggered a scan at server startup,
so a fresh server's model registry stayed empty until a client manually called
the rescan endpoint, even though `ServerConfig.model_dirs` is already
configured by that point. Per the project owner, models must always be scanned
on startup, so `P18-C3` wires that trigger into `backend/main.rs` directly,
reusing `P18-C2`'s own internal trigger function rather than duplicating it.
Separately, the same audit pass that found this confirmed `P18-D2`'s premise —
that "the pool's existing respawn-on-exit path (also Phase 8) already produces
exactly the restart behavior needed" — was not actually true when `P18-D2` was
authored: Phase 8's original task set never wired `RespawnPolicy` into a real
respawn loop. That gap is now closed by `P900`-adjacent additions to Phase 8
itself (`P8-E4`/`P8-E5`, inserted before Phase 8 executes), so `P18-D2`'s own
text requires no change — it will simply become true once Phase 8 runs with
its corrected task set, rather than needing its own retrofit here.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Final AppState fields | P18-A1 | `hardware`, `env_report` |
| B | System handlers | P18-B1 … P18-B2 | `/v1/system`, `/v1/system/env`, then `/v1/system/versions` |
| C | Model handlers | P18-C1 … P18-C3 | `model_store` field + list/get, rescan, then startup auto-scan |
| D | Worker handlers | P18-D1 … P18-D2 | List, then restart via existing respawn machinery |
| E | Job deletion | P18-E1 … P18-E2 | Single-job delete, then bulk clear |
| F | OpenAPI | P18-F1 … P18-F2 | Real generation, then the CI gate |
| G | Proof | P18-G1 | The phase's Runnable Proof |

---

## Prerequisites

`AppState` must already hold `scheduler`/`workers`/`db`/`broadcaster`/`node_registry`/
`artifact_store`/`config` per Phases 11, 14, 15, 16. `ModelStore` must exist per
Phase 6 (P6-A3). `WorkerPool`'s respawn machinery must work per Phase 8 (P8-D1).
Job cancellation's terminal-status conflict pattern must exist per Phase 17 (P17-C1).

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §13.2` | P18-A1 | `AppState`'s complete, final ten-field shape |
| `ANVILML_DESIGN.md §13.4` | All handler tasks | Exact route paths, methods, and response shapes |
| `ANVILML_DESIGN.md §13.5` | P18-E1, P18-E2 | Status code mapping, especially `409`/`400` |
| `ENVIRONMENT.md` Gate 2 | P18-F1, P18-F2 | The exact `openapi-drift` gate command |

---

## Task Descriptions

### Group A — Final AppState fields

#### P18-A1: anvilml-server: AppState gains hardware, env_report fields (final)

**Goal:** Complete `AppState` with its last two fields, finishing the
incremental build-up that's spanned Phases 11, 14, 15, and 16.

**Files to create or modify:**
- `crates/anvilml-server/src/state.rs` — adds `hardware`, `env_report`.
- `backend/src/main.rs` — populates both at startup.

**Key implementation notes:**
- `hardware` is populated once via `detect_all_devices()` (Phase 5) — ongoing VRAM
  refresh during dispatch is a separate concern not addressed by this task.
- `env_report`'s initial populate is best-effort, using the preflight checks
  `ENVIRONMENT.md §5` describes — this task does not need to build a full preflight
  subsystem if one doesn't already exist; it wires whatever's available.

**Acceptance criterion:**
```bash
cargo test -p anvilml-server --test state_tests
cargo build -p anvilml
# -> both exit 0
```

---

### Group B — System handlers

#### P18-B1: anvilml-server: GET /v1/system, /v1/system/env handlers

**Goal:** Expose the hardware snapshot and environment report over HTTP for the
first time.

**Files to create or modify:**
- `crates/anvilml-server/src/handlers/system.rs` — `get_system()`,
  `get_system_env()`.

**Key implementation notes:**
- Both are one-line delegations to a read-locked `AppState` field, per
  `ANVILML_DESIGN.md §3.3`. `/v1/system/versions` is explicitly deferred to the
  next task.

**Acceptance criterion:**
```bash
cargo test -p anvilml-server --test system_tests
# -> >=4 tests, exits 0
```

#### P18-B2: anvilml-server: GET /v1/system/versions handler + ComponentVersions type

**Goal:** Complete the system handler group with a per-component version report.

**Files to create or modify:**
- `crates/anvilml-server/src/handlers/system.rs` — adds `get_system_versions()`,
  `ComponentVersions`.

**Key implementation notes:**
- `ComponentVersions` is a new HTTP-response-layer struct in this file, not
  `anvilml-core` — it doesn't represent a domain concept, just a response shape.
- `python_version`/`torch_version` are `Option<String>` sourced from `env_report`
  — correctly `None` if the preflight hasn't populated them yet.

**Acceptance criterion:**
```bash
cargo test -p anvilml-server --test system_tests
# -> >=7 tests total in the file, exits 0
```

---

### Group C — Model handlers

#### P18-C1: anvilml-server: AppState gains model_store; GET /v1/models, /v1/models/:id

**Goal:** Connect the HTTP layer to the model registry for the first time.

**Files to create or modify:**
- `crates/anvilml-server/src/state.rs` — adds `model_store`.
- `crates/anvilml-server/src/handlers/models.rs` — new; `list_models()`,
  `get_model()`.

**Key implementation notes:**
- No prior phase wired `ModelStore` into `AppState` — this task is the first to do
  so. `POST /v1/models/rescan` is explicitly deferred to the next task.

**Acceptance criterion:**
```bash
cargo test -p anvilml-server --test models_tests
# -> >=4 tests, exits 0
```

#### P18-C2: anvilml-server: POST /v1/models/rescan handler

**Goal:** Expose the model directory rescan trigger, returning immediately rather
than blocking on the scan.

**Files to create or modify:**
- `crates/anvilml-server/src/handlers/models.rs` — adds `rescan_models()`.

**Key implementation notes:**
- The scan runs in a spawned tokio task; the HTTP response (`202`) returns
  immediately, never waiting on scan completion — a directory with many model
  files should not make this endpoint slow to respond.

**Acceptance criterion:**
```bash
cargo test -p anvilml-server --test models_tests
# -> >=6 tests total in the file, exits 0
```

---

#### P18-C3: backend: trigger model scan on server startup

**Goal:** Close an audit-found gap — `ModelScanner::scan_dir()` (Phase 6) was
only ever reachable through `P18-C2`'s rescan endpoint, so a fresh server's
model registry stayed empty until a client manually triggered a rescan. Per
the project owner: models must always be scanned on startup.

**Files to create or modify:**
- `backend/src/main.rs` — triggers a background scan in the default
  (non-`hw-probe`) startup path.

**Key implementation notes:**
- Trigger after `AppState` construction (`P18-A1`, so `model_store` exists)
  and before binding the TCP listener. Reuse `P18-C2`'s internal trigger
  function rather than duplicating its `tokio::spawn` + `scan_dir()` call —
  the same non-blocking, fire-and-forget contract applies here: startup must
  not wait on scan completion.
- Log scan start at `INFO`.

**Acceptance criterion:**
```bash
cargo test -p anvilml --test startup_scan_tests
# -> exits 0; >=2 tests: spawning the built binary against a temp model_dir
#    with a planted model file lists that model via GET /v1/models within a
#    bounded poll window, with no /v1/models/rescan call made
```

---

### Group D — Worker handlers

#### P18-D1: anvilml-server: GET /v1/workers list handler

**Goal:** Expose the worker pool's current state over HTTP.

**Files to create or modify:**
- `crates/anvilml-server/src/handlers/workers.rs` — new; `list_workers()`.

**Key implementation notes:**
- Reuses a `list()`-equivalent method on `WorkerPool` if Phase 16's `stats_tick`
  task already added one — check before adding a duplicate.
- `POST /v1/workers/:id/restart` is explicitly deferred to the next task.

**Acceptance criterion:**
```bash
cargo test -p anvilml-server --features mock-hardware --test workers_tests
# -> >=3 tests, exits 0
```

#### P18-D2: anvilml-server: POST /v1/workers/:id/restart via existing respawn machinery

**Goal:** Expose worker restart, composed entirely from machinery that already
exists — no new restart-specific logic.

**Files to create or modify:**
- `crates/anvilml-server/src/handlers/workers.rs` — adds `restart_worker()`.

**Key implementation notes:**
- This is **composition, not new logic**: `request_shutdown()` (Phase 8) plus the
  pool's existing respawn-on-exit path (also Phase 8) already produces exactly the
  restart behavior needed. Building a separate, parallel "restart" code path would
  duplicate logic that already exists and works.

**Acceptance criterion:**
```bash
cargo test -p anvilml-server --features mock-hardware --test workers_tests
# -> >=6 tests total in the file, exits 0
```

---

### Group E — Job deletion

#### P18-E1: anvilml-server: DELETE /v1/jobs/:id single-job delete handler

**Goal:** Expose terminal-job deletion, reusing the same conflict pattern
cancellation established in Phase 17.

**Files to create or modify:**
- `crates/anvilml-server/src/handlers/jobs.rs` — adds `delete_job()`.

**Key implementation notes:**
- `409` on a non-terminal job (the same pattern Phase 17's cancel handler uses),
  `404` on an unknown ID, `204` on success. Deletes both the job row and its
  associated artifacts together.
- Bulk clear is explicitly deferred to the next task.

**Acceptance criterion:**
```bash
cargo test -p anvilml-server --test jobs_tests
# -> >=18 tests total in the file, exits 0
```

#### P18-E2: anvilml-server: DELETE /v1/jobs bulk clear handler

**Goal:** Complete job deletion with a bulk-clear endpoint, built on the same
per-job delete logic rather than a divergent implementation.

**Files to create or modify:**
- `crates/anvilml-server/src/handlers/jobs.rs` — adds `bulk_clear_jobs()`.

**Key implementation notes:**
- Reuses P18-E1's per-job delete logic for each matching job — not a separate
  bulk-delete SQL path that could drift from the single-job behavior.
- `400` on an unrecognized `status` query value.

**Acceptance criterion:**
```bash
cargo test -p anvilml-server --test jobs_tests
# -> >=22 tests total in the file, exits 0
```

---

### Group F — OpenAPI

#### P18-F1: anvilml-openapi: real OpenAPI generation from utoipa annotations

**Goal:** Replace Phase 1's stub print with real OpenAPI spec generation,
annotating every handler across the entire server crate.

**Files to create or modify:**
- `crates/anvilml-openapi/src/main.rs` — real generation logic.
- `crates/anvilml-server/src/handlers/*.rs` — `#[utoipa::path(...)]` annotations on
  every handler, across every file (a workspace-wide pass).

**Key implementation notes:**
- Every response type involved is already `ToSchema` from its Phase 3 definition —
  no new derive work needed there, just the path annotations and the `OpenApi`
  struct tying them together.
- This is the first time `api/openapi.json` has real content — confirm a second
  run produces an identical file (idempotency matters for the CI gate).

**Acceptance criterion:**
```bash
cargo run -p anvilml-openapi
# -> exits 0, produces a valid, non-empty api/openapi.json with every §13.4 route
```

#### P18-F2: CI: wire openapi-drift job to real generation + diff check

**Goal:** Make the `openapi-drift` CI job — a placeholder echo since Phase 1 —
actually check something.

**Files to create or modify:**
- `.github/workflows/ci.yml` — `openapi-drift` job's real steps.
- `api/openapi.json` — committed for the first time with real content.

**Key implementation notes:**
- The gate is exactly `cargo run -p anvilml-openapi && git diff --exit-code
  api/openapi.json` per `ENVIRONMENT.md` Gate 2 — a stale committed spec fails this
  check, forcing regeneration before merge.

**Acceptance criterion:**
```bash
grep -c 'openapi-drift' .github/workflows/ci.yml
# -> job exists with real (non-echo) steps
cargo run -p anvilml-openapi && git diff --exit-code api/openapi.json
# -> exits 0 locally
```

---

### Group G — Proof

#### P18-G1: Runnable Proof: live binary serves /v1/system and /v1/workers with real data

**Goal:** Produce this phase's Runnable Proof, confirming two of the
newly-completed routes work against a live server.

**Files to create or modify:**
- None. This task runs the already-built binary; see Acceptance Criterion.

**Key implementation notes:**
- This closes out the entire REST surface from `§13.4` — every route now backed by
  real, non-stub logic, across eighteen phases of incremental construction.

**Acceptance criterion:**
```bash
cargo build --release -p anvilml --features mock-hardware
ANVILML_MOCK_DEVICE_TYPE=cuda ./target/release/anvilml &
sleep 2
curl -s http://127.0.0.1:8488/v1/system | python3 -c "import sys,json; d=json.load(sys.stdin); assert len(d['gpus'])>=1"
curl -s http://127.0.0.1:8488/v1/workers | python3 -c "import sys,json; assert isinstance(json.load(sys.stdin), list)"
# -> both exit 0
kill %1
```

---

## Phase Acceptance Criteria

```bash
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test --workspace --features mock-hardware
cargo run -p anvilml-openapi && git diff --exit-code api/openapi.json

# Runnable Proof (manual):
cargo build --release -p anvilml --features mock-hardware
ANVILML_MOCK_DEVICE_TYPE=cuda ./target/release/anvilml &
sleep 2
curl -s http://127.0.0.1:8488/v1/system | python3 -c "import sys,json; d=json.load(sys.stdin); assert len(d['gpus'])>=1"
curl -s http://127.0.0.1:8488/v1/workers | python3 -c "import sys,json; assert isinstance(json.load(sys.stdin), list)"
# -> both exit 0
kill %1
```

---

## Known Constraints and Gotchas

- `POST /v1/workers/:id/restart` must not introduce a new restart code path — it
  composes `request_shutdown()` with the pool's existing respawn machinery, both
  already built in Phase 8.
- `DELETE /v1/jobs` (bulk clear) must reuse `DELETE /v1/jobs/:id`'s per-job delete
  logic, not a separate, potentially-diverging bulk SQL operation.
- `api/openapi.json` becomes a genuinely meaningful, committed artifact starting
  with this phase — a future handler signature change without a regenerated spec
  will now actually fail CI, where before it silently passed against the Phase 1
  stub.
- This phase completes `AppState`'s full ten-field shape and the entire REST route
  table — no further incremental `AppState` field growth or new REST routes are
  expected outside of what later architecture-loading phases might add.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 18 — HTTP/WebSocket Server Completion

**Capability proved:** `GET /v1/system` and `GET /v1/workers` are both live over a
real HTTP server with real (mock-detected) data — the final two routes confirming
the complete REST surface from `ANVILML_DESIGN.md §13.4` is now backed entirely by
real, non-stub logic.

\`\`\`bash
# Runnable Proof (manual):
cargo build --release -p anvilml --features mock-hardware
ANVILML_MOCK_DEVICE_TYPE=cuda ./target/release/anvilml &
sleep 2
curl -s http://127.0.0.1:8488/v1/system | python3 -c "import sys,json; d=json.load(sys.stdin); assert len(d['gpus'])>=1"
curl -s http://127.0.0.1:8488/v1/workers | python3 -c "import sys,json; assert isinstance(json.load(sys.stdin), list)"
# -> both exit 0
kill %1
\`\`\`
```