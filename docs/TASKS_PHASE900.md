# Tasks: Phase 900 — Spec-Drift Retrofit: /health Body & Missing ToSchema

**Phase:** 900
**Name:** Spec-Drift Retrofit: /health Body & Missing ToSchema
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 3, 6

---

## Overview

This is a retrofit phase, inserted out-of-band between Phase 6 and Phase 7, per
`FORGE_TASK_AUTHORING_SPEC.md §6`'s numbering convention for retrofit phases
(900–999, ordering enforced via `prereqs` rather than numeric position). It exists
because a manual audit against `ANVILML_DESIGN.md`, conducted while implementation
sat at P6-A2, found three independent instances of the same root-cause defect: a
task's `context` field silently dropped part of the design doc's spec when it was
written, and the resulting implementation, test, and review all faithfully matched
the (defective) task rather than the design doc. None of the three was caught by
any existing gate, because each one is internally consistent — the code does
exactly what its own task asked for, just not what `ANVILML_DESIGN.md` specifies.

The three findings are: (1) `GET /health` (Phase 1, `P1-D1`) returns a bare `200`
with no body, where `ANVILML_DESIGN.md §13.4` specifies a JSON body of
`{ status, version, uptime_s }`; (2) `Job`, `JobStatus`, and `JobSettings` (Phase 3,
`P3-A1`) are missing the `ToSchema` derive that `ANVILML_DESIGN.md §5.3` requires on
all three; (3) `ModelMeta`, `ModelKind`, `ModelDtype`, and `ModelFormat` (Phase 3,
`P3-A2`) are missing the `ToSchema` derive that `ANVILML_DESIGN.md §5.4` requires on
all four. The two `ToSchema` omissions matter beyond cosmetic correctness: Phase 1's
`anvilml-openapi` stub (`P1-B6`) exists specifically to later read every type's
`ToSchema` impl and emit `api/openapi.json` — a type missing the derive at that
point would be either silently absent from the generated spec or a hard compile
error, depending on how that future task is written. Closing the gap now, before
Phase 7's IPC layer and Phase 8's worker pool start depending on these same
`anvilml-core` types in more places, is cheaper than retrofitting it later.

This phase is scheduled after Phase 6 completes and before Phase 7 begins. It has
no functional dependency on Phase 6's registry/artifacts work — the three fixes
touch only `anvilml-server`'s health handler and `anvilml-core`'s `job`/`model`
type modules — but the project owner requested it land in this exact window, so
`P900-A1`'s `prereqs` explicitly includes Phase 6's two leaf tasks (`P6-A9`,
`P6-B3`) alongside its real dependency (`P1-D1`), and Phase 7's first task
(`P7-A1`) has had `P900-A3` added to its own `prereqs` so The Forge cannot begin
Phase 7 until this phase's last task is complete. At the start of this phase, the
three defects above are live in the repository. At the end: `/health` returns the
spec-correct JSON body, and every domain type the design doc says should derive
`ToSchema` does.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Spec-drift fixes | P900-A1 … P900-A3 | `/health` JSON body, `Job`-family `ToSchema`, `Model`-family `ToSchema` |

---

## Prerequisites

Phase 6 must be complete — specifically `anvilml-registry`'s `lib.rs` re-export
pass (`P6-A9`) and `anvilml-artifacts`'s `ArtifactStore::list` (`P6-B3`), the two
tasks nothing else in Phase 6 depends on. `crates/anvilml-server/src/handlers/health.rs`
must exist exactly as `P1-D1` left it (Phase 1). `crates/anvilml-core/src/types/job.rs`
and `crates/anvilml-core/src/types/model.rs` must exist exactly as `P3-A1` and
`P3-A2` left them (Phase 3). `anvilml-core/Cargo.toml` must already depend on
`utoipa` with the `uuid`/`chrono` features (established in Phase 3, used unchanged
here — no new dependency is introduced by this phase).

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §13.4` | P900-A1 | `/health` success response shape: `200 { status, version, uptime_s }` |
| `ANVILML_DESIGN.md §5.3` | P900-A2 | `Job`, `JobStatus`, `JobSettings` each derive `ToSchema` |
| `ANVILML_DESIGN.md §5.4` | P900-A3 | `ModelMeta`, `ModelKind`, `ModelDtype`, `ModelFormat` each derive `ToSchema` |

---

## Task Descriptions

### Group A — Spec-drift fixes

#### P900-A1: anvilml-server: /health returns ANVILML_DESIGN.md §13.4 JSON body

**Goal:** Make `GET /health` return the JSON body `ANVILML_DESIGN.md §13.4`
specifies, closing the gap between the live route and its own design spec before
any external client or later phase comes to depend on the bare-status-code shape.

**Files to create or modify:**
- `crates/anvilml-server/src/handlers/health.rs` — replace the `StatusCode`-only
  return with a `HealthResponse` struct serialised as the handler's JSON body.
- `backend/src/main.rs` — capture a process-start `Instant` once at startup and
  make it available to the handler (via `axum::extract::State` or a static
  `OnceLock`) so `uptime_s` is real elapsed time.
- `crates/anvilml-server/Cargo.toml` — add `serde`'s `derive` feature if not
  already available transitively.
- `crates/anvilml-server/tests/health_tests.rs` — extend the existing test to
  assert on the three JSON fields.

**Key implementation notes:**
- `HealthResponse{status: String, version: String, uptime_s: u64}`, deriving
  `Debug, Clone, Serialize`. `status` is always the literal `"ok"` — the handler
  only runs while the process is alive, so there is no other value to report.
- `version` comes from `env!("CARGO_PKG_VERSION")` — no new version-tracking
  mechanism is needed.
- `uptime_s` must be derived from a real `Instant` captured at process start, not
  a hardcoded `0` — a static `0` would technically match the JSON shape while
  still failing the spec's intent.
- The handler's return type changes from `StatusCode` to `Json<HealthResponse>`;
  axum's `IntoResponse` for `Json<T>` already produces the `200` status, so no
  explicit status code is set.

**Acceptance criterion:**
```bash
cargo test -p anvilml-server --test health_tests
# -> exits 0, with assertions on status/version/uptime_s fields in addition to
#    the existing 200-status assertion
```

---

#### P900-A2: anvilml-core: add missing ToSchema to Job/JobStatus/JobSettings

**Goal:** Add the `ToSchema` derive that `ANVILML_DESIGN.md §5.3` specifies for
`Job`, `JobStatus`, and `JobSettings` but that the live `job.rs` is missing
entirely — confirmed by the complete absence of a `utoipa` import in the file.

**Files to create or modify:**
- `crates/anvilml-core/src/types/job.rs` — add `use utoipa::ToSchema;` and append
  `ToSchema` to the derive list on all three types.

**Key implementation notes:**
- This is additive-derive-only: no field, variant, or `serde` attribute changes
  on any of the three types. Behaviour is unchanged; only the `ToSchema` impl is
  added.
- `Job` and `JobSettings` keep their current derive lists otherwise unchanged;
  `JobStatus` keeps its existing `Copy, PartialEq, Eq`.
- `Debug` and `Clone` are already present on all three types, satisfying
  `ToSchema`'s macro requirements — no other derive needs to be added alongside it.
- `ArtifactMeta` (`P3-A3`, the task immediately after the one that introduced this
  gap) correctly includes `ToSchema`, confirming the omission is isolated to this
  file rather than a project-wide pattern.

**Acceptance criterion:**
```bash
cargo test -p anvilml-core --test job_tests
# -> exits 0, all existing tests unaffected
cargo doc -p anvilml-core --no-deps
# -> exits 0, confirming the new ToSchema impls compile
```

---

#### P900-A3: anvilml-core: add missing ToSchema to ModelMeta/ModelKind/ModelDtype/ModelFormat

**Goal:** Add the `ToSchema` derive that `ANVILML_DESIGN.md §5.4` specifies for
`ModelMeta`, `ModelKind`, `ModelDtype`, and `ModelFormat` but that the live
`model.rs` is missing entirely — the second and last instance of this omission
pattern found by the audit.

**Files to create or modify:**
- `crates/anvilml-core/src/types/model.rs` — add `use utoipa::ToSchema;` and
  append `ToSchema` to the derive list on all four types.

**Key implementation notes:**
- Additive-derive-only, identical in nature to `P900-A2`: no field, variant, or
  `serde` attribute changes. The `#[serde(rename_all = "snake_case")]` attributes
  on the three enums are untouched.
- This is the last task in the phase — it closes with a full-workspace regression
  run, since it is the last point at which any `anvilml-core` type derive changes
  before Phase 7 begins building on top of this crate.
- Confirmed isolated: `hardware.rs`, `worker.rs`, `node.rs`, `artifact.rs`, and
  `events.rs` all already correctly derive `ToSchema` where the design doc
  requires it — only `job.rs` (`P900-A2`) and `model.rs` (this task) were missing
  it.

**Acceptance criterion:**
```bash
cargo test -p anvilml-core --test model_tests
# -> exits 0, all existing tests unaffected
cargo doc -p anvilml-core --no-deps
# -> exits 0
cargo test --workspace --features mock-hardware
# -> exits 0, phase-closing full-workspace regression check
```

---

## Phase Acceptance Criteria

```bash
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test --workspace --features mock-hardware
cargo doc -p anvilml-core --no-deps

# Runnable Proof (manual): /health now returns a real JSON body, not just a bare
# 200 status code.
cargo build --release -p anvilml --features mock-hardware
./target/release/anvilml &
sleep 1
curl -s http://127.0.0.1:8488/health | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['status']=='ok' and isinstance(d['version'],str) and isinstance(d['uptime_s'],int)"
# -> exits 0; the JSON body contains status="ok", a string version, and an
#    integer uptime_s — previously this body was empty
kill %1
```

---

## Known Constraints and Gotchas

- This phase's tasks intentionally do not touch `anvilml-server/src/handlers/mod.rs`
  or `lib.rs`'s `build_router()` signature beyond what `P900-A1` requires to thread
  the start time through — no new routes are added.
- `P900-A1` is tagged `"breaking"` because it changes `health()`'s return type
  from `StatusCode` to `Json<HealthResponse>` — any code written between Phase 1
  and now that matched on the handler's old return type (none is expected to
  exist) would need updating.
- `P900-A2` and `P900-A3` are tagged `"refactor"` per `FORGE_TASK_AUTHORING_SPEC.md
  §13` — they make zero observable behaviour change; only a derive is added.
  Per `§9`'s narrow exemption, no live-instance Runnable Proof is required for
  these two individually, but the phase as a whole still provides one via
  `P900-A1`, so the phase-level exemption does not apply here.
- `P7-A1`'s `prereqs` have been updated to include `P900-A3` (in addition to its
  existing `P3-A11` prereq) so that Phase 7 cannot begin until this phase
  completes, per the project owner's explicit scheduling request — this is a
  cross-phase edit made as part of authoring this phase, not a separate task.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 900 — Spec-Drift Retrofit: /health Body & Missing ToSchema

**Capability proved:** `GET /health` returns the `ANVILML_DESIGN.md §13.4`-specified
JSON body (`status`, `version`, `uptime_s`) instead of a bare `200` with no body.

```bash
cargo build --release -p anvilml --features mock-hardware
./target/release/anvilml &
sleep 1
curl -s http://127.0.0.1:8488/health | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['status']=='ok' and isinstance(d['version'],str) and isinstance(d['uptime_s'],int)"
# -> exits 0; status/version/uptime_s all present and correctly typed
kill %1
```
```
