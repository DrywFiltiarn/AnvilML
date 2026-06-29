# Tasks: Phase 900 — Spec-Drift & Logging Retrofit: tracing-subscriber, /health Body & Missing ToSchema

**Phase:** 900
**Name:** Spec-Drift & Logging Retrofit: tracing-subscriber, /health Body & Missing ToSchema
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 3, 6

---

## Overview

This is a retrofit phase, inserted out-of-band between Phase 6 and Phase 7, per
`FORGE_TASK_AUTHORING_SPEC.md §6`'s numbering convention for retrofit phases
(900–999, ordering enforced via `prereqs` rather than numeric position). It exists
because a manual audit against `ANVILML_DESIGN.md` and `ENVIRONMENT.md`, conducted
while implementation sat at P6-A2, found five independent instances of the same
root-cause defect: a task's `context` field silently dropped part of the design
doc's (or environment doc's) spec when it was written, and the resulting
implementation, test, and review all faithfully matched the (defective) task
rather than the authoritative document. None of the five was caught by any
existing gate, because each one is internally consistent — the code does exactly
what its own task asked for, just not what the governing document specifies.

The five findings are: (0) no task in Phases 1–6 ever wired `tracing-subscriber`
into the binary — `tracing` (the facade) is a dependency and `tracing::info!`/
`debug!` calls exist (`P1-D1`'s listening log, `P1-A3`'s shutdown log), but with no
registered `Subscriber` every one of those calls is a silent no-op, and the
`ANVILML_LOG`/`RUST_LOG` precedence `ENVIRONMENT.md §3.3` documents is read by
nothing; (1) `GET /health` (Phase 1, `P1-D1`) returns a bare `200` with no body,
where `ANVILML_DESIGN.md §13.4` specifies a JSON body of `{ status, version,
uptime_s }`; (2) `Job`, `JobStatus`, and `JobSettings` (Phase 3, `P3-A1`) are
missing the `ToSchema` derive that `ANVILML_DESIGN.md §5.3` requires on all three;
(3) `ModelMeta`, `ModelKind`, `ModelDtype`, and `ModelFormat` (Phase 3, `P3-A2`)
are missing the `ToSchema` derive that `ANVILML_DESIGN.md §5.4` requires on all
four; (4) `ENVIRONMENT.md §3.3` also documents a `--log-format plain|json` CLI
flag (default `plain`) controlling tracing's output format, but no task in any
phase ever added it — `cli.rs` has no such field, and this gap was found
specifically because it sits in the same section of the same document as finding
0, surfaced once that finding was already under review. The logging gap (finding
0) is the highest-priority of the five — it was escalated by the project owner as
a blocking operational defect, since it leaves the running binary silent on the
terminal with no way to diagnose startup or shutdown behaviour, and is therefore
scheduled as this phase's first task ahead of the other findings. Finding 4
depends on finding 0's fix existing first (there is no subscriber to format until
then) and is scheduled last in this phase, after the unrelated `/health`/`ToSchema`
work, since nothing else in the phase depends on it. The two `ToSchema` omissions
matter beyond cosmetic correctness: Phase 1's `anvilml-openapi` stub (`P1-B6`)
exists specifically to later read every type's `ToSchema` impl and emit
`api/openapi.json` — a type missing the derive at that point would be either
silently absent from the generated spec or a hard compile error, depending on how
that future task is written. Closing all five gaps now, before Phase 7's IPC layer
and Phase 8's worker pool start depending on these same `anvilml-core` types and
the same silent-logging binary in more places, is cheaper than retrofitting it
later.

This phase is scheduled after Phase 6 completes and before Phase 7 begins. None of
the five fixes has a functional dependency on Phase 6's registry/artifacts work —
they touch `backend`'s entry point, `anvilml-server`'s health handler, and
`anvilml-core`'s `job`/`model` type modules — but the project owner requested the
phase land in this exact window, so `P900-A2`'s `prereqs` explicitly includes
Phase 6's two leaf tasks (`P6-A9`, `P6-B3`) alongside its real dependencies
(`P900-A1`, `P1-D1`), and Phase 7's first task (`P7-A1`) has had its `prereqs`
updated to point at this phase's new final task (`P900-A5`, after this revision
appended the log-format flag fix as the phase's new last task) so The Forge cannot
begin Phase 7 until this phase's last task is complete. At the start of this
phase, all five defects above are live in the repository. At the end: the binary
emits real log output honouring `ANVILML_LOG`/`RUST_LOG` in either plain-text or
JSON format via `--log-format`, `/health` returns the spec-correct JSON body, and
every domain type the design doc says should derive `ToSchema` does.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Spec-drift & logging fixes | P900-A1 … P900-A5 | `tracing-subscriber` wiring, `/health` JSON body, `Job`-family `ToSchema`, `Model`-family `ToSchema`, `--log-format` flag |

---

## Prerequisites

`backend/Cargo.toml` and `backend/src/main.rs` must exist exactly as `P1-A2`/`P1-A3`
left them (Phase 1) — no subscriber initialization present, `tracing` declared as
a dependency but `tracing-subscriber` absent from the dependency tree entirely.
Phase 6 must be complete — specifically `anvilml-registry`'s `lib.rs` re-export
pass (`P6-A9`) and `anvilml-artifacts`'s `ArtifactStore::list` (`P6-B3`), the two
tasks nothing else in Phase 6 depends on. `crates/anvilml-server/src/handlers/health.rs`
must exist exactly as `P1-D1` left it (Phase 1). `crates/anvilml-core/src/types/job.rs`
and `crates/anvilml-core/src/types/model.rs` must exist exactly as `P3-A1` and
`P3-A2` left them (Phase 3). `anvilml-core/Cargo.toml` must already depend on
`utoipa` with the `uuid`/`chrono` features (established in Phase 3, used unchanged
here — no new dependency is introduced by `P900-A3`/`P900-A4`).

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ENVIRONMENT.md §3.3` | P900-A1 | `ANVILML_LOG` takes precedence over `RUST_LOG`; both default to `info` when unset |
| `ANVILML_DESIGN.md §13.4` | P900-A2 | `/health` success response shape: `200 { status, version, uptime_s }` |
| `ANVILML_DESIGN.md §5.3` | P900-A3 | `Job`, `JobStatus`, `JobSettings` each derive `ToSchema` |
| `ANVILML_DESIGN.md §5.4` | P900-A4 | `ModelMeta`, `ModelKind`, `ModelDtype`, `ModelFormat` each derive `ToSchema` |
| `ENVIRONMENT.md §3.3` | P900-A5 | `--log-format plain\|json` CLI flag, default `plain`, no env-var equivalent |

---

## Task Descriptions

### Group A — Spec-drift & logging fixes

#### P900-A1: backend: wire tracing-subscriber, ANVILML_LOG/RUST_LOG never read

**Goal:** Make the binary actually emit log output. This is the priority fix in
this phase — escalated ahead of the three originally-scheduled findings because a
silent binary with no diagnostic output is a blocking operational defect, not a
cosmetic spec mismatch.

**Files to create or modify:**
- `backend/Cargo.toml` — add `tracing-subscriber` with the `env-filter` feature.
- `backend/src/main.rs` — initialize the subscriber as the first statement in
  `main()`, before CLI parsing or config loading.

**Key implementation notes:**
- `tracing` (the facade crate providing the `tracing::info!`/`debug!` macros) has
  been a dependency since Phase 1, but no `Subscriber` has ever been registered —
  `tracing-subscriber` does not appear anywhere in `Cargo.lock`. Without a
  registered subscriber, every `tracing::*!` call in the codebase is a no-op; this
  is why `P1-D1`'s `"listening"` log and `P1-A3`'s shutdown log produce no visible
  output despite the calls existing in source.
- Filter precedence must match `ENVIRONMENT.md §3.3` exactly: try `ANVILML_LOG`
  first, fall back to `RUST_LOG`, default to `"info"` if neither is set.
- This must run before any other startup work so that config-loading and
  hardware-detection log lines (already written elsewhere in the codebase) become
  visible immediately once this task lands, with no further changes required at
  those call sites.

**Acceptance criterion:**
```bash
cargo test -p anvilml --test logging_tests
# -> exits 0; >=3 tests confirming ANVILML_LOG=debug and RUST_LOG=debug both
#    produce non-empty stderr, and that ANVILML_LOG wins when both are set
```

---

#### P900-A2: anvilml-server: /health returns ANVILML_DESIGN.md §13.4 JSON body

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

#### P900-A3: anvilml-core: add missing ToSchema to Job/JobStatus/JobSettings

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

#### P900-A4: anvilml-core: add missing ToSchema to ModelMeta/ModelKind/ModelDtype/ModelFormat

**Goal:** Add the `ToSchema` derive that `ANVILML_DESIGN.md §5.4` specifies for
`ModelMeta`, `ModelKind`, `ModelDtype`, and `ModelFormat` but that the live
`model.rs` is missing entirely — the last instance of this omission pattern found
by the audit, and the final task in this phase.

**Files to create or modify:**
- `crates/anvilml-core/src/types/model.rs` — add `use utoipa::ToSchema;` and
  append `ToSchema` to the derive list on all four types.

**Key implementation notes:**
- Additive-derive-only, identical in nature to `P900-A3`: no field, variant, or
  `serde` attribute changes. The `#[serde(rename_all = "snake_case")]` attributes
  on the three enums are untouched.
- This is the last task in the phase — it closes with a full-workspace regression
  run, since it is the last point at which any `anvilml-core` type derive changes
  before Phase 7 begins building on top of this crate.
- Confirmed isolated: `hardware.rs`, `worker.rs`, `node.rs`, `artifact.rs`, and
  `events.rs` all already correctly derive `ToSchema` where the design doc
  requires it — only `job.rs` (`P900-A3`) and `model.rs` (this task) were missing
  it.

**Acceptance criterion:**
```bash
cargo test -p anvilml-core --test model_tests
# -> exits 0, all existing tests unaffected
cargo doc -p anvilml-core --no-deps
# -> exits 0
```

---

#### P900-A5: backend: add --log-format plain|json CLI flag per ENVIRONMENT.md §3.3

**Goal:** Add the `--log-format plain|json` CLI flag `ENVIRONMENT.md §3.3`
documents but that no task in any phase ever implemented — found in the same
audit pass as `P900-A1`, once that finding was already under review, since both
gaps sit in the same section of the same document. This is the phase's final
task and closes with the full-workspace regression run.

**Files to create or modify:**
- `backend/src/cli.rs` — add a `log_format` field to `Cli`.
- `backend/src/main.rs` — branch the subscriber builder `P900-A1` introduced on
  the new flag's value.
- `backend/tests/logging_tests.rs` — extend `P900-A1`'s test file with flag-format
  coverage rather than creating a second logging test file.

**Key implementation notes:**
- `log_format: Option<String>` via `#[arg(long, default_value = "plain")]`,
  validated against exactly `"plain"` or `"json"` — any other value exits non-zero
  with clap's usual usage output, matching the existing CLI error convention
  rather than introducing a new one.
- The branch reuses `P900-A1`'s `EnvFilter` precedence unchanged — `--log-format`
  only selects the output encoding (`fmt()` vs `fmt().json()`), it does not affect
  which lines are emitted or at what level.
- `ENVIRONMENT.md §3.3` is explicit that output format is controlled by this CLI
  flag, "not by an environment variable" — no `ANVILML_LOG_FORMAT`-style variable
  should be introduced as an alternative or fallback.
- This task depends on `P900-A1` (the subscriber must exist before its output
  format is selectable) and is sequenced after `P900-A4` purely because nothing in
  the phase depends on it — the `/health` and `ToSchema` fixes are unrelated to
  logging and were scheduled first.

**Acceptance criterion:**
```bash
cargo test -p anvilml --test logging_tests
# -> exits 0; >=4 tests total in the file (P900-A1's 3 plus this task's coverage):
#    --log-format=json produces valid-JSON stderr lines, plain default unaffected,
#    an invalid value exits non-zero
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

# Runnable Proof (manual): the binary now emits visible log output honouring
# ANVILML_LOG/RUST_LOG, selectable as plain text or JSON via --log-format, and
# /health returns a real JSON body instead of a bare 200 status code.
cargo build --release -p anvilml --features mock-hardware
ANVILML_LOG=debug ./target/release/anvilml --log-format json &
sleep 1
curl -s http://127.0.0.1:8488/health | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['status']=='ok' and isinstance(d['version'],str) and isinstance(d['uptime_s'],int)"
# -> exits 0; the JSON body contains status="ok", a string version, and an
#    integer uptime_s — previously this body was empty; stderr above also shows
#    real DEBUG-level log lines as JSON — previously stderr was silent regardless
#    of --log-format, which did not exist
kill %1
```

---

## Known Constraints and Gotchas

- `P900-A1` is the priority-escalated fix in this phase and intentionally has no
  dependency on `P900-A2`/`P900-A3`/`P900-A4` beyond being scheduled first — it
  could in principle land independently, but is kept in this phase per the
  project owner's instruction to batch it alongside the previously-found
  spec-drift defects under one retrofit phase.
- This phase's tasks intentionally do not touch `anvilml-server/src/handlers/mod.rs`
  or `lib.rs`'s `build_router()` signature beyond what `P900-A2` requires to thread
  the start time through — no new routes are added.
- `P900-A1` is tagged `"breaking"` because it changes `main()`'s startup sequence
  and the binary's observable stderr behaviour for the first time. `P900-A2` is
  tagged `"breaking"` because it changes `health()`'s return type from
  `StatusCode` to `Json<HealthResponse>` — any code written between Phase 1 and
  now that matched on the handler's old return type (none is expected to exist)
  would need updating. `P900-A5` is tagged `"breaking"` because it changes
  `Cli`'s field set and `main()`'s subscriber-construction branch a second time.
- `P900-A3` and `P900-A4` are tagged `"refactor"` per `FORGE_TASK_AUTHORING_SPEC.md
  §13` — they make zero observable behaviour change; only a derive is added.
  Per `§9`'s narrow exemption, no live-instance Runnable Proof is required for
  these two individually, but the phase as a whole still provides one via
  `P900-A1`/`P900-A2`/`P900-A5`, so the phase-level exemption does not apply here.
- `P900-A5` was found during the same audit pass as `P900-A1`, after that finding
  was already scheduled — both gaps sit in `ENVIRONMENT.md §3.3`. It is sequenced
  last in this phase (depending on `P900-A1` and, by execution order, `P900-A4`)
  because nothing else in the phase depends on it, and it now carries the
  phase-closing full-workspace regression check that `P900-A4` carried before
  this revision appended `P900-A5` after it.
- `P7-A1`'s `prereqs` have been updated to reference `P900-A5` (in addition to its
  existing `P3-A11` prereq), replacing the prior reference to `P900-A4` from
  before this revision appended the log-format flag fix as the new last task — so
  that Phase 7 cannot begin until this phase completes, per the project owner's
  explicit scheduling request. This is a cross-phase edit made as part of
  authoring this phase, not a separate task.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 900 — Spec-Drift & Logging Retrofit: tracing-subscriber, /health Body & Missing ToSchema

**Capability proved:** The `anvilml` binary emits real log output honouring
`ANVILML_LOG`/`RUST_LOG`, selectable as plain text or JSON via `--log-format`
(previously silent regardless of either variable, and the flag did not exist),
and `GET /health` returns the `ANVILML_DESIGN.md §13.4`-specified JSON body
(`status`, `version`, `uptime_s`) instead of a bare `200` with no body.

```bash
cargo build --release -p anvilml --features mock-hardware
ANVILML_LOG=debug ./target/release/anvilml --log-format json &
sleep 1
curl -s http://127.0.0.1:8488/health | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['status']=='ok' and isinstance(d['version'],str) and isinstance(d['uptime_s'],int)"
# -> exits 0; status/version/uptime_s all present and correctly typed; stderr
#    shows real DEBUG-level output as JSON from the same run
kill %1
```
```