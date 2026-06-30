# Tasks: Phase 900 — Spec-Drift & Logging Retrofit: tracing-subscriber, /health Body, Missing ToSchema & DB Wiring

**Phase:** 900
**Name:** Spec-Drift & Logging Retrofit: tracing-subscriber, /health Body, Missing ToSchema & DB Wiring
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

The five original findings are: (0) no task in Phases 1–6 ever wired `tracing-subscriber`
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

This revision adds three more tasks to the phase, found by two later, independent
checks rather than the original five-finding audit above. `P900-A1`'s original
`context` asked for a `backend/tests/logging_tests.rs` that spawns the built
binary and reads its stderr, but never specified how to obtain a reliable path to
that binary inside an automated `cargo test` run — the agent executing it spent an
extended session rediscovering, by trial and error, that `cargo test`'s
`--features` flag does not propagate to a separately-spawned child process and
that a manually-resolved `target/debug` path is fragile across build profiles.
This is the same context-field-silently-drops-detail root cause the rest of this
phase exists to fix, just discovered later, against this phase's own first task
rather than against Phases 1–6. `P900-A1`'s `context` and this document's matching
H4 subsection have been corrected in place (not superseded by a new task ID,
since the original was never successfully completed) to mandate the
`Command::new(env!("CARGO_BIN_EXE_anvilml"))` pattern `backend/tests/
hw_probe_help_test.rs` (`P5-A5`) already establishes elsewhere in the codebase,
which the original `context` should have referenced from the start. Correcting
`P900-A1` narrowed its acceptance from three tests to two, dropping the
"`ANVILML_LOG` wins when both are set" precedence assertion to stay under the
1000-character `context` cap (`§11`) once the corrected instructions were added —
per `§10`'s sizing rule, a task too large to specify within the cap must split
rather than silently lose scope, so that assertion is restored as the new
`P900-A8`, a small companion task with no production-code changes of its own.

Separately, a third check — verifying whether `backend`'s entry point actually
exercises Phase 6's registry work, not just whether Phase 6's library code
compiled and passed its own unit tests — found that `create_pool()`,
`DeviceCapabilityStore`, and `SeedLoader` (all complete and unit-tested by the end
of Phase 6) are never called from anywhere outside `anvilml-registry`'s own
source and tests. `backend/Cargo.toml` has no dependency on `anvilml-registry` at
all, and `anvilml-server`'s `Cargo.toml` lists it but never imports it. No
database file, migration, or seed row is ever produced by running the actual
`anvilml` binary, despite all of the code that would do so existing and passing
its own tests in isolation — the same internally-consistent-but-spec-violating
pattern the rest of this phase addresses, just at the level of "was this ever
wired into the binary" rather than "does this match the design doc's field list."
`P900-A6` wires `create_pool()` (pool creation plus the migration runner) into
`main()`'s default startup path; `P900-A7` wires `SeedLoader::run()` immediately
after. Both deliberately avoid introducing any part of `AppState` — per
`ANVILML_DESIGN.md §13.2`, `AppState` is a much larger struct (`scheduler`,
`workers`, `broadcaster`, `node_registry`, none of which exist yet) that Phase 11
begins building incrementally; constructing a partial stand-in now would itself
become a second instance of the spec-drift this phase exists to close.

A fourth, independent check — tracing every type built in Phases 1–6 forward to
confirm later tasks' assumptions about its shape actually hold, rather than only
auditing each task against the design doc at the point it was authored — found
that `P3-A6`'s `EnvReport` has only 3 fields (`python_version: String`,
`torch_version: Option<String>`, `torch_importable: bool`), where
`ANVILML_DESIGN.md`'s type definitions specify 7: `python_path`,
`python_version` (as `Option<String>`, not `String`), `torch_version`,
`provisioning: ProvisioningState`, `preflight_ok`, `reason`, and
`node_types: Vec<NodeTypeDescriptor>`. `ProvisioningState` itself exists as a
type but the current `EnvReport` never references it — dead code as
implemented — and its variant names (`NotStarted`, `InProgress`, `Complete`,
`Failed`) don't match the doc's (`Ready`, `Provisioning`, `Failed`,
`NotStarted`) either. Two already-authored later tasks already assume the
doc's shape: `P18-A1` states "`AppState` now has every field §13.2 specifies,"
relying on `EnvReport` correctly carrying provisioning/preflight data, and
`P28-B1` explicitly instructs populating "`EnvReport`'s
`python_version/torch_version/preflight_ok/reason`" — fields that do not exist
on the struct `P3-A6` actually built; `P28-B1` would fail to compile against
the current struct once it executes. `P900-A9` rewrites `EnvReport` to the
doc-correct 7-field shape; `P900-A10` is a small companion fixing
`ProvisioningState`'s variant names, sequenced after `P900-A9` since it
populates the field that task adds. Neither task is on the critical path to
Phase 7 — `P7-A1` continues to point at `P900-A5` — but both must land before
Phase 18 and Phase 28 execute, since those phases' already-authored tasks
depend on the corrected shape.

This phase is scheduled after Phase 6 completes and before Phase 7 begins. None of
the original five fixes has a functional dependency on Phase 6's registry/artifacts
work — they touch `backend`'s entry point, `anvilml-server`'s health handler, and
`anvilml-core`'s `job`/`model` type modules — but the project owner requested the
phase land in this exact window, so `P900-A2`'s `prereqs` explicitly includes
Phase 6's two leaf tasks (`P6-A9`, `P6-B3`) alongside its real dependencies
(`P900-A1`, `P1-D1`). `P900-A6` and `P900-A7`, added by this revision, do have a
direct functional dependency on Phase 6 — they wire `anvilml-registry`'s
`create_pool()` and `SeedLoader` into the binary — so both are prereq'd on
`P6-B3` directly rather than only inheriting it transitively. Phase 7's first task
(`P7-A1`) continues to point at `P900-A5`; the three tasks this revision adds
(`P900-A6`, `P900-A7`, `P900-A8`) are not on that critical path; The Forge may run
them in any order relative to `P7-A1` so long as their own `prereqs` are
satisfied. At the start of this phase, all five original defects, plus the
unwired database layer, are live in the repository. At the end: the binary emits
real log output honouring `ANVILML_LOG`/`RUST_LOG` in either plain-text or JSON
format via `--log-format`, `/health` returns the spec-correct JSON body, every
domain type the design doc says should derive `ToSchema` does, and the binary
creates its SQLite database, runs migrations, and loads the device-capability
seed data on every real startup.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Spec-drift, logging & DB-wiring fixes | P900-A1 … P900-A10 | `tracing-subscriber` wiring, `/health` JSON body, `Job`-family `ToSchema`, `Model`-family `ToSchema`, `--log-format` flag, `create_pool()` wiring, `SeedLoader` wiring, `ANVILML_LOG` precedence verification, `EnvReport` field shape, `ProvisioningState` variant names |

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
here — no new dependency is introduced by `P900-A3`/`P900-A4`). `P900-A1` (as
corrected by this revision) requires `backend/tests/hw_probe_help_test.rs`
(`P5-A5`) to exist unchanged as the pattern reference for `Command::new(env!(
"CARGO_BIN_EXE_anvilml"))`. `P900-A6` and `P900-A7` require Phase 6's
`anvilml-registry::create_pool()` (`P6-A2`), `DeviceCapabilityStore` (`P6-A5`),
and `SeedLoader` (`P6-A7`) to exist exactly as Phase 6 left them — fully
implemented and unit-tested, but not yet referenced by `backend` or
`anvilml-server`'s `Cargo.toml` dependency graph. `database/seeds/devices.sql`
(`P6-A8`) must already contain the converted PCI-ID rows `P900-A7`'s test
asserts against. `P900-A9` and `P900-A10` require `crates/anvilml-core/src/types/worker.rs`
to exist exactly as `P3-A6` left it (Phase 3) — `EnvReport` with 3 fields,
`ProvisioningState` with its current 4 variants — and `NodeTypeDescriptor`
(`P3-A7`) to already exist, since `P900-A9`'s corrected `EnvReport` references it.

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ENVIRONMENT.md §3.3` | P900-A1, P900-A8 | `ANVILML_LOG` takes precedence over `RUST_LOG`; both default to `info` when unset |
| `ANVILML_DESIGN.md §13.4` | P900-A2 | `/health` success response shape: `200 { status, version, uptime_s }` |
| `ANVILML_DESIGN.md §5.3` | P900-A3 | `Job`, `JobStatus`, `JobSettings` each derive `ToSchema` |
| `ANVILML_DESIGN.md §5.4` | P900-A4 | `ModelMeta`, `ModelKind`, `ModelDtype`, `ModelFormat` each derive `ToSchema` |
| `ENVIRONMENT.md §3.3` | P900-A5 | `--log-format plain\|json` CLI flag, default `plain`, no env-var equivalent |
| `ANVILML_DESIGN.md §7.1`/§7.5 | P900-A6, P900-A7 | `create_pool()`'s migration runner; `SeedLoader`'s hash-gated idempotent apply |
| `ANVILML_DESIGN.md §13.2` | P900-A6, P900-A7 | `AppState` is explicitly out of scope for both tasks — confirms the field list neither task may introduce yet |
| `ANVILML_DESIGN.md` type definitions (§9) | P900-A9 | `EnvReport`'s 7-field shape: `python_path`, `python_version`, `torch_version`, `provisioning`, `preflight_ok`, `reason`, `node_types` |
| `ANVILML_DESIGN.md` type definitions (§9) | P900-A10 | `ProvisioningState`'s variants: `Ready`, `Provisioning`, `Failed`, `NotStarted` |

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
- `backend/tests/logging_tests.rs` — new file; spawns the built binary per the
  pattern below.

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
- **Corrected by this revision** (the original instructions caused the first
  attempt at this task to loop indefinitely trying to locate a working binary
  path): tests MUST mirror `backend/tests/hw_probe_help_test.rs`'s existing
  pattern exactly — `Command::new(env!("CARGO_BIN_EXE_anvilml"))`, `.args(["hw-probe"])`
  (no server bind needed, so no port/socket setup), `.env("ANVILML_LOG", "debug")`
  (or `"RUST_LOG"`), then assert `!output.stderr.is_empty()`. `CARGO_BIN_EXE_anvilml`
  is a compile-time environment variable Cargo provides automatically to
  integration tests in the `anvilml` package; it always points at the binary
  built in the same profile and with the same features as the test itself — do
  NOT construct a `target/debug/anvilml` or `target/release/anvilml` path by
  hand, and do NOT pass `--features` to the `cargo test` invocation expecting it
  to reach the spawned child binary, since `cargo test` builds the package binary
  separately from the test binary and the flag does not cross that boundary.

**Acceptance criterion:**
```bash
cargo test -p anvilml --test logging_tests
# -> exits 0; >=2 tests confirming ANVILML_LOG=debug and RUST_LOG=debug each
#    independently produce non-empty stderr from the spawned hw-probe subcommand
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
# -> exits 0; >=4 tests total in the file (P900-A1's 2 plus this task's coverage):
#    --log-format=json produces valid-JSON stderr lines, plain default unaffected,
#    an invalid value exits non-zero
cargo test --workspace --features mock-hardware
# -> exits 0, phase-closing full-workspace regression check
```

---

#### P900-A6: backend: wire create_pool() into server startup, no AppState yet

**Goal:** Make the running binary actually create its SQLite database and run
migrations. Phase 6 built `create_pool()` (pool creation plus the migration
runner) fully and unit-tested it, but `backend` was never given a dependency on
`anvilml-registry`, so no code path in the running server ever calls it — no `.db`
file is ever produced.

**Files to create or modify:**
- `backend/Cargo.toml` — add `anvilml-registry` as a dependency.
- `backend/src/main.rs` — call `create_pool()` in the default (non-`hw-probe`)
  startup path.
- `backend/tests/db_startup_tests.rs` — new file.

**Key implementation notes:**
- In `main()`'s default path, after config load and before binding the TCP
  listener, call `anvilml_registry::create_pool(&config.db_path).await`. On
  `Err`, `eprintln!` the error and `std::process::exit(1)` before binding any
  socket — the same failure pattern config-loading already uses.
- Keep the resulting `SqlitePool` local to `main()`. Do NOT introduce any part of
  `AppState` in this task — per `ANVILML_DESIGN.md §13.2`, `AppState` is a much
  larger struct (`scheduler`, `workers`, `broadcaster`, `node_registry`) that
  doesn't exist until Phase 11 begins building it incrementally; a partial
  stand-in here would itself be a new instance of this phase's root-cause defect.
- `config.db_path` already exists on `ServerConfig` (Phase 2, `P2-A2`) and is
  already threaded through the TOML/env/CLI precedence chain — no new config
  field is introduced by this task.

**Acceptance criterion:**
```bash
cargo test -p anvilml --test db_startup_tests
# -> exits 0; >=2 tests spawning the built binary against a temp db_path,
#    asserting the .db file is created and both the models and
#    device_capabilities tables exist in it
```

---

#### P900-A7: backend: wire SeedLoader::run() for database/seeds/devices.sql at startup

**Goal:** Make the running binary actually populate `device_capabilities`. Phase 6
built `SeedLoader` fully (hash-gated, idempotent, unit-tested), but nothing
outside `anvilml-registry`'s own tests ever calls it — the table stays empty even
once `P900-A6`'s pool and migrations run.

**Files to create or modify:**
- `backend/src/main.rs` — call `SeedLoader::run()` immediately after `P900-A6`'s
  `create_pool()` call.
- `backend/tests/db_startup_tests.rs` — extends `P900-A6`'s file; does not create
  a second test file.

**Key implementation notes:**
- Construct `SeedLoader::new(pool.clone())` and call `.run("devices.sql",
  Path::new("database/seeds/devices.sql")).await` directly after `P900-A6`'s pool
  is created. Log applied/skipped at `INFO`. On `Err`, `eprintln!` and
  `std::process::exit(1)` before binding any socket, matching `P900-A6`'s
  pattern.
- `SeedLoader::run()` is already idempotent via its `_seed_log` hash-bookkeeping
  table (Phase 6, `P6-A6`/`P6-A7`) — this task only has to call it, not
  re-implement any of that logic.

**Acceptance criterion:**
```bash
cargo test -p anvilml --test db_startup_tests
# -> exits 0; >=3 tests total in the file (P900-A6's 2 plus this task's
#    coverage): first run populates device_capabilities (row count > 0,
#    matching devices.sql's INSERT count), a second run against the same
#    db_path is idempotent (no duplicate rows, no error), a missing or
#    malformed seed file causes startup to exit non-zero
cargo test --workspace --features mock-hardware
# -> exits 0, regression check
```

---

#### P900-A8: backend: verify ANVILML_LOG precedence over RUST_LOG (P900-A1 companion)

**Goal:** Restore the precedence assertion `P900-A1`'s original three-test
acceptance specified but which had to be dropped when that task's `context` was
corrected and needed to stay under the 1000-character cap. No production code
changes — `P900-A1`'s filter chain already implements `ANVILML_LOG` taking
precedence over `RUST_LOG`; this task only adds the missing verification.

**Files to create or modify:**
- `backend/tests/logging_tests.rs` — extends `P900-A1`'s file with one new test;
  does not create a second test file.

**Key implementation notes:**
- Mirror `P900-A1`'s exact `Command::new(env!("CARGO_BIN_EXE_anvilml"))` +
  `.args(["hw-probe"])` pattern, but set both `.env("ANVILML_LOG", "debug")` and
  `.env("RUST_LOG", "error")` on the same `Command`.
- Assert `!output.stderr.is_empty()`. `RUST_LOG=error` alone would normally
  suppress `debug`-level output entirely, so non-empty stderr in this combined
  case proves `ANVILML_LOG="debug"` was the filter actually applied, not
  `RUST_LOG="error"` — a stricter assertion (matching a literal `DEBUG`-level
  line) is preferable if the log output format makes that reliable, but
  non-empty stderr is the minimum bar this task must clear.

**Acceptance criterion:**
```bash
cargo test -p anvilml --test logging_tests
# -> exits 0; backend/tests/logging_tests.rs now has >=3 tests total
```

---

#### P900-A9: anvilml-core: fix EnvReport's field shape to match ANVILML_DESIGN.md

**Goal:** Fix a third independent context-field-drops-spec-detail defect, found
by tracing Phase 3's `EnvReport` forward to confirm later tasks' assumptions
about its shape actually hold. `P3-A6`'s implemented `EnvReport` has only 3
fields; the design doc specifies 7. Two already-authored later tasks (`P18-A1`,
`P28-B1`) assume the doc's shape — `P28-B1` will not compile against the
current struct once it executes.

**Files to create or modify:**
- `crates/anvilml-core/src/types/worker.rs` — rewrite `EnvReport`'s field list.
- `crates/anvilml-core/tests/worker_tests.rs` — update (not remove) the
  `EnvReport` roundtrip test for the new shape.

**Key implementation notes:**
- The doc-correct shape is `EnvReport { python_path: Option<String>,
  python_version: Option<String>, torch_version: Option<String>, provisioning:
  ProvisioningState, preflight_ok: bool, reason: Option<String>, node_types:
  Vec<NodeTypeDescriptor> }` — note `python_version` changes from `String` to
  `Option<String>` (the current type is also wrong, not just incomplete).
- `NodeTypeDescriptor` already exists from `P3-A7`, the task immediately after
  the one that introduced this gap — no new dependency is needed.
- `ProvisioningState`'s own variant-name mismatch against the design doc is
  `P900-A10`'s scope, not this task's — do not rename any variants here, only
  add the `provisioning: ProvisioningState` field using the type as it
  currently exists.

**Acceptance criterion:**
```bash
cargo test -p anvilml-core --test worker_tests
# -> exits 0, EnvReport's roundtrip test updated for all 7 fields
cargo doc -p anvilml-core --no-deps
# -> exits 0
```

---

#### P900-A10: anvilml-core: fix ProvisioningState's variant names to match ANVILML_DESIGN.md

**Goal:** Companion to `P900-A9` — `ProvisioningState`'s variants don't match
the design doc either, and since `P900-A9` just wired the type into `EnvReport`
as a real field, the mismatch is no longer just dead-code drift.

**Files to create or modify:**
- `crates/anvilml-core/src/types/worker.rs` — rename two variants.
- `crates/anvilml-core/tests/worker_tests.rs` — update (not remove) the
  `ProvisioningState` roundtrip test for the renamed variants.

**Key implementation notes:**
- Rename `InProgress` → `Provisioning` and `Complete` → `Ready`; keep
  `NotStarted` and `Failed` as-is. This is a pure rename — the enum's role
  (tracked by `P28-A1`'s startup provisioning check) is unchanged.
- The `#[serde(rename_all = "snake_case")]` attribute means the JSON wire
  values change too: `"in_progress"` → `"provisioning"`, `"complete"` →
  `"ready"` — update the test's expected strings accordingly.
- `P900-A9` must land first — this task's renamed variants populate the
  `provisioning` field `P900-A9` adds to `EnvReport`.

**Acceptance criterion:**
```bash
cargo test -p anvilml-core --test worker_tests
# -> exits 0, ProvisioningState's roundtrip test updated for the renamed
#    variants and their new snake_case JSON strings
cargo doc -p anvilml-core --no-deps
# -> exits 0
```

---

## Phase Acceptance Criteria

```bash
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test --workspace --features mock-hardware
cargo doc -p anvilml-core --no-deps

# Runnable Proof (manual): the binary now emits visible log output honouring
# ANVILML_LOG/RUST_LOG, selectable as plain text or JSON via --log-format,
# /health returns a real JSON body instead of a bare 200 status code, and the
# binary creates its SQLite database, runs migrations, and loads the
# device-capability seed data on startup.
rm -f /tmp/anvilml-proof.db
cargo build --release -p anvilml --features mock-hardware
ANVILML_LOG=debug ANVILML_DB_PATH=/tmp/anvilml-proof.db ./target/release/anvilml --log-format json &
sleep 1
curl -s http://127.0.0.1:8488/health | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['status']=='ok' and isinstance(d['version'],str) and isinstance(d['uptime_s'],int)"
# -> exits 0; the JSON body contains status="ok", a string version, and an
#    integer uptime_s — previously this body was empty; stderr above also shows
#    real DEBUG-level log lines as JSON — previously stderr was silent regardless
#    of --log-format, which did not exist
kill %1
sleep 1
test -f /tmp/anvilml-proof.db
sqlite3 /tmp/anvilml-proof.db "SELECT COUNT(*) FROM device_capabilities;" | grep -qv '^0$'
# -> both exit 0; the .db file exists and device_capabilities has at least one
#    row — previously no .db file was ever created by the running binary at all
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
- `P900-A1`'s `context` and this document's matching subsection were corrected in
  place by this revision after the original instructions caused an indefinite
  debugging loop — `cargo test -p anvilml`'s `--features` flag does not reach a
  separately-spawned child binary, and a hand-resolved `target/debug` path is
  fragile. The corrected instructions mandate `Command::new(env!(
  "CARGO_BIN_EXE_anvilml"))`, mirroring the existing, working
  `backend/tests/hw_probe_help_test.rs` pattern from `P5-A5` — any future task
  spawning the built binary from a Rust integration test should follow the same
  pattern rather than reconstructing a path manually.
- `P900-A6` and `P900-A7` deliberately do not touch `AppState` or
  `anvilml-server` at all — they only add a dependency and two function calls to
  `backend/src/main.rs`. The `SqlitePool` they create is not stored anywhere
  beyond `main()`'s local scope; a later phase (Phase 11 onward, per
  `ANVILML_DESIGN.md §13.2`) is responsible for threading a pool into `AppState`
  once that struct exists, and should reuse `P900-A6`'s `create_pool()` call site
  rather than duplicating it.
- `P900-A8` makes no production-code change — it exists solely because `P900-A1`'s
  acceptance criterion had to be narrowed from three tests to two to fit the
  1000-character `context` cap once the `CARGO_BIN_EXE` correction was added.
- `P900-A9` and `P900-A10` were found by tracing Phase 3's types forward to their
  later consumers rather than by re-auditing Phase 3 against the design doc in
  isolation — the same method that found the `create_pool()`/`SeedLoader` gap
  (`P900-A6`/`P900-A7`). `P28-B1`, as currently authored, will not compile until
  `P900-A9` lands; this is flagged here so a future session executing Phase 28
  does not mistake the resulting compile error for a new defect.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 900 — Spec-Drift & Logging Retrofit: tracing-subscriber, /health Body, Missing ToSchema & DB Wiring

**Capability proved:** The `anvilml` binary emits real log output honouring
`ANVILML_LOG`/`RUST_LOG`, selectable as plain text or JSON via `--log-format`
(previously silent regardless of either variable, and the flag did not exist);
`GET /health` returns the `ANVILML_DESIGN.md §13.4`-specified JSON body
(`status`, `version`, `uptime_s`) instead of a bare `200` with no body; and the
binary now creates its SQLite database, runs migrations, and loads the
device-capability seed data on every real startup (previously no `.db` file was
ever produced, despite the pool/migration/seed code existing and passing its own
unit tests since Phase 6).

```bash
rm -f /tmp/anvilml-proof.db
cargo build --release -p anvilml --features mock-hardware
ANVILML_LOG=debug ANVILML_DB_PATH=/tmp/anvilml-proof.db ./target/release/anvilml --log-format json &
sleep 1
curl -s http://127.0.0.1:8488/health | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['status']=='ok' and isinstance(d['version'],str) and isinstance(d['uptime_s'],int)"
# -> exits 0; status/version/uptime_s all present and correctly typed; stderr
#    shows real DEBUG-level output as JSON from the same run
kill %1
sleep 1
test -f /tmp/anvilml-proof.db
sqlite3 /tmp/anvilml-proof.db "SELECT COUNT(*) FROM device_capabilities;" | grep -qv '^0$'
# -> both exit 0; the .db file exists and device_capabilities has at least one
#    row — previously no .db file was ever created by the running binary
```
```