# Tasks: Phase 1 — Repository Scaffold

**Phase:** 1
**Name:** Repository Scaffold
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** none

---

## Overview

This phase creates the AnvilML repository's skeleton: the Cargo workspace, every crate
as an empty (doc-comment-only) stub in correct dependency order, the `backend` binary
with a minimal CLI and a single `/health` endpoint, the checked-in reference config
file, and the full CI workflow matrix. No business logic exists anywhere in this
phase — every crate compiles, but only `anvilml-server`'s `/health` handler does
anything observable.

This phase exists first because every later phase needs a compiling workspace to add
code to, and the crate dependency graph (`ANVILML_DESIGN.md §3.2`) must be established
correctly from the start — adding crates out of order, or with the wrong path
dependencies, is exactly the kind of mistake that is expensive to unwind later. The
codebase starts as nothing (a near-empty repository) and ends this phase as a
buildable, testable, CI-validated skeleton with one real HTTP route.

At the end of this phase: `cargo build --workspace --features mock-hardware` succeeds;
the `anvilml` binary starts, binds an HTTP port, and answers `GET /health` with `200`;
all CI jobs are defined and green (most as placeholders for subsystems that don't
exist yet). Phase 2 depends on `anvilml-core` existing as a buildable (if empty) crate
to add config and domain types into.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Workspace & binary | P1-A1 … P1-A3 | Cargo workspace root, toolchain pin, backend binary with CLI parsing and a shutdown-signal stub |
| B | Crate stubs | P1-B1 … P1-B6 | Every crate in the dependency graph created as an empty, doc-commented stub, added to the workspace in dependency order |
| C | Reference config | P1-C1 | Checked-in `anvilml.toml` with the two fields that exist at this phase |
| D | Health endpoint | P1-D1 … P1-D2 | `GET /health` handler, router wiring, and the phase's Runnable Proof |
| E | CI | P1-E1 … P1-E2 | Full CI workflow file: real rust-test matrix job now, placeholder worker-test/drift jobs for subsystems not yet built |

---

## Prerequisites

None. This is the first phase.

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §3.2` | P1-B1 … P1-B6 | Crate dependency graph — no crate may depend on a crate above it in the graph |
| `ANVILML_DESIGN.md §3.1` / `ARCHITECTURE.md §2` | P1-A1 … P1-B6 | Repository layout — exact file and directory names |
| `ENVIRONMENT.md §1` | P1-A1 | Rust toolchain pin: 1.96.0, edition 2024 |
| `ANVILML_DESIGN.md §18.3` / `ENVIRONMENT.md §6 Step 11` | P1-E1, P1-E2 | CI job matrix shape, `fmt --check` is Linux-only |

---

## Task Descriptions

### Group A — Workspace & binary

#### P1-A1: Workspace: Cargo.toml, toolchain pin, gitattributes

**Goal:** Establish the Cargo workspace root and the pinned toolchain so every
subsequent crate has a place to register itself and a fixed Rust version/edition to
compile against.

**Files to create or modify:**
- `Cargo.toml` — workspace root; `members = ["backend"]` initially; `[workspace.package]` with `version = "0.1.0"`, `edition = "2024"`, `rust-version = "1.96.0"`.
- `rust-toolchain.toml` — pins `channel = "1.96.0"`, `components = ["rustfmt", "clippy"]`, `targets = ["x86_64-pc-windows-gnu"]`.
- `.gitattributes` — line-ending rules: `*.sh`/`*.py`/`*.rs` as LF, `*.ps1` as CRLF.

**Key implementation notes:**
- `members` only lists `backend` at this point — crate paths under `crates/*` are
  added incrementally by the tasks that create those crates (P1-B1 onward), since
  Cargo errors on a workspace member path that doesn't exist yet.
- `rust-version` in `[workspace.package]` is the single source of truth other crates
  inherit via `rust-version.workspace = true` — do not restate the version string in
  any other `Cargo.toml`.

**Acceptance criterion:**
```bash
test -s Cargo.toml && test -s rust-toolchain.toml && test -s .gitattributes
rustc --version  # -> rustc 1.96.0 (...)
```

#### P1-A2: backend: main.rs, cli.rs stubs, binary compiles

**Goal:** Produce a buildable `anvilml` binary with CLI argument parsing for the three
flags the design doc's config precedence chain (`ANVILML_DESIGN.md §15`) ultimately
needs at the top of its precedence order.

**Files to create or modify:**
- `backend/Cargo.toml` — package `anvilml`, inherits workspace version/edition.
- `backend/src/main.rs` — `fn main()` calls `cli::parse()`, prints a scaffold message.
- `backend/src/cli.rs` — `clap`-derived `Cli` struct: `host` (default `127.0.0.1`),
  `port` (default `8488`), `config: Option<String>`.

**Key implementation notes:**
- Resolve the current `clap` version via the crates.io MCP/registry tool before
  writing it into `Cargo.toml` — do not use a version recalled from training data
  (`FORGE_AGENT_RULES.md §6`).
- Use `clap`'s derive feature (`features = ["derive"]`) for the `Cli` struct, not the
  builder API — keeps `cli.rs` declarative and short.

**Acceptance criterion:**
```bash
cargo build -p anvilml
# -> exit 0
./target/debug/anvilml --help
# -> shows --host, --port, --config
```

#### P1-A3: backend: shutdown.rs signal handler stub

**Goal:** Add a cross-platform shutdown-signal future the binary can race against,
laying the groundwork for the full graceful-shutdown sequence (`ANVILML_DESIGN.md
§19.3`) that a later phase will build out in full.

**Files to create or modify:**
- `backend/src/shutdown.rs` — `async fn wait_for_shutdown_signal()`.
- `backend/src/main.rs` — converted to `#[tokio::main] async fn main()`; calls the new
  function after the scaffold print.
- `backend/Cargo.toml` — adds `tokio` with `features = ["full"]`.

**Key implementation notes:**
- `wait_for_shutdown_signal()` awaits `tokio::signal::ctrl_c()`. This already works
  identically on Linux and Windows via tokio's own cross-platform handling — no `cfg`
  branch for SIGINT vs Ctrl-C is needed at this stage; the SIGTERM-specific Unix
  handling and the 30-second worker-drain sequence (`ANVILML_DESIGN.md §19.3`) are
  explicitly out of scope here and belong to the phase that introduces `WorkerPool`.
- Resolve `tokio`'s current version live via the registry MCP tool, not from memory.

**Acceptance criterion:**
```bash
cargo build -p anvilml
# -> exit 0
cargo test -p anvilml --test shutdown_tests
# -> >=1 test, exits 0
```

---

### Group B — Crate stubs

#### P1-B1: anvilml-core: empty crate, compiles, in workspace

**Goal:** Create the dependency-graph root crate as an empty, doc-commented stub so
every other crate has something to depend on starting with the very next task.

**Files to create or modify:**
- `crates/anvilml-core/Cargo.toml` — workspace-inherited version/edition, zero
  dependencies.
- `crates/anvilml-core/src/lib.rs` — crate-level `//!` doc comment only.
- `Cargo.toml` (root) — add `"crates/anvilml-core"` to `members`.

**Key implementation notes:**
- `lib.rs` content is exactly the crate doc comment stating "Pure domain types,
  config schema, error enum. Zero I/O. Zero async. No tokio, no sqlx, no network." —
  no submodule declarations yet; those arrive with the types in Phase 2/3.
- File must stay under the 80-line hard cap (`ANVILML_DESIGN.md §4.1`) — at this size
  it will be roughly 3 lines, nowhere near the limit, which is itself a useful sanity
  check that nothing extra was added.

**Acceptance criterion:**
```bash
cargo build -p anvilml-core
# -> exit 0
```

#### P1-B2: anvilml-hardware: empty crate stub + mock-hardware feature decl

**Goal:** Stub the hardware-detection crate and declare the `mock-hardware` feature
flag at its point of origin, since every later crate's feature-forwarding (`anvilml-worker`,
`anvilml-scheduler`, `anvilml-server`, `backend`) depends on this flag existing here first.

**Files to create or modify:**
- `crates/anvilml-hardware/Cargo.toml` — path dependency on `anvilml-core`; `[features]
  mock-hardware = []`.
- `crates/anvilml-hardware/src/lib.rs` — crate doc comment only.
- `Cargo.toml` (root) — add to `members`.

**Key implementation notes:**
- `mock-hardware = []` is intentionally empty at this point — it gates no code yet,
  since no detector implementations exist. It exists now purely so later crates can
  already write `mock-hardware = ["anvilml-hardware/mock-hardware"]` without a
  forward reference to a feature that doesn't exist.

**Acceptance criterion:**
```bash
cargo build -p anvilml-hardware
cargo build -p anvilml-hardware --features mock-hardware
# -> both exit 0
```

#### P1-B3: anvilml-registry, anvilml-artifacts: empty crate stubs

**Goal:** Stub the two crates that own persisted state (models and artifacts), kept
as a single task since both are equally trivial stubs with the same shape and the
same single dependency.

**Files to create or modify:**
- `crates/anvilml-registry/Cargo.toml`, `crates/anvilml-registry/src/lib.rs`.
- `crates/anvilml-artifacts/Cargo.toml`, `crates/anvilml-artifacts/src/lib.rs`.
- `Cargo.toml` (root) — add both to `members`.

**Key implementation notes:**
- Neither crate depends on `sqlx` yet — that dependency is added by the task that
  implements `db.rs`/`store.rs` in the Model Registry phase, not here, to avoid an
  unused dependency sitting in the manifest for multiple phases.
- Each crate depends on `anvilml-core` only, matching `ANVILML_DESIGN.md §3.2`'s graph.

**Acceptance criterion:**
```bash
cargo build -p anvilml-registry -p anvilml-artifacts
# -> exit 0
```

#### P1-B4: anvilml-ipc, anvilml-worker: empty crate stubs

**Goal:** Stub the IPC and worker-supervision crates, establishing the
`anvilml-worker → anvilml-ipc` dependency edge and the `mock-hardware` forwarding
pattern at the first crate where it actually applies.

**Files to create or modify:**
- `crates/anvilml-ipc/Cargo.toml`, `crates/anvilml-ipc/src/lib.rs`.
- `crates/anvilml-worker/Cargo.toml` (depends on `anvilml-ipc`, `anvilml-hardware`,
  `anvilml-core`; `[features] mock-hardware = ["anvilml-hardware/mock-hardware"]`),
  `crates/anvilml-worker/src/lib.rs`.
- `Cargo.toml` (root) — add both to `members`.

**Key implementation notes:**
- `anvilml-ipc` depends only on `anvilml-core`, per the graph — it must never gain a
  dependency on `anvilml-worker` or `anvilml-hardware` (`ANVILML_DESIGN.md §8.1`).
- The feature-forwarding line in `anvilml-worker/Cargo.toml` is the exact pattern
  every downstream crate repeats (`ARCHITECTURE.md §5`).

**Acceptance criterion:**
```bash
cargo build -p anvilml-ipc -p anvilml-worker --features mock-hardware
# -> exit 0
```

#### P1-B5: anvilml-scheduler, anvilml-server: empty crate stubs

**Goal:** Stub the two highest-level library crates, completing the dependency chain
up to (but not including) the top-level binary's own wiring.

**Files to create or modify:**
- `crates/anvilml-scheduler/Cargo.toml` (depends on `anvilml-worker`,
  `anvilml-registry`, `anvilml-artifacts`, `anvilml-core`; forwards `mock-hardware`),
  `crates/anvilml-scheduler/src/lib.rs`.
- `crates/anvilml-server/Cargo.toml` (depends on all of the above plus `axum`;
  forwards `mock-hardware`), `crates/anvilml-server/src/lib.rs`.
- `Cargo.toml` (root) — add both to `members`.
- `backend/Cargo.toml` — add path dependencies on `anvilml-scheduler` and
  `anvilml-server` (needed by the next group's router wiring).

**Key implementation notes:**
- Resolve `axum`'s current version live via the registry MCP tool before pinning it
  — do not use a recalled version.
- This is the task where the full workspace first builds end-to-end with the feature
  flag forwarded all the way from `anvilml-hardware` to `backend`.

**Acceptance criterion:**
```bash
cargo build --workspace --features mock-hardware
# -> exit 0
```

#### P1-B6: anvilml-openapi: build-time stub binary

**Goal:** Create the build-time OpenAPI-generation binary as a stub, and the `api/`
output directory it will eventually write into, so the `openapi-drift` CI gate has a
real (if trivial) command to run from day one.

**Files to create or modify:**
- `crates/anvilml-openapi/Cargo.toml` — binary crate depending on `anvilml-core`,
  `anvilml-server`.
- `crates/anvilml-openapi/src/main.rs` — prints a stub message, exits 0.
- `api/.gitkeep` — placeholder so the directory exists in git before
  `api/openapi.json` is generated by a later task.
- `Cargo.toml` (root) — add to `members`.

**Key implementation notes:**
- Real OpenAPI emission (reading `anvilml-server`'s route annotations and writing
  `api/openapi.json`) is explicitly out of scope here — `anvilml-server` has no
  handlers with OpenAPI annotations yet at this point in the phase (P1-D1 runs after
  this task and only adds a bare `/health` route with no schema).

**Acceptance criterion:**
```bash
cargo run -p anvilml-openapi
# -> exit 0, prints "openapi generation stub"
```

---

### Group C — Reference config

#### P1-C1: anvilml.toml checked-in reference config (scaffold defaults)

**Goal:** Create the canonical reference config file with exactly the fields that
exist at this point in the build, establishing the file early so it grows in lockstep
with `ServerConfig` rather than being retrofitted once many fields already exist.

**Files to create or modify:**
- `anvilml.toml` (repo root) — `host = "127.0.0.1"`, `port = 8488`, with a comment
  header noting its role as the `config_reference` test's source of truth.

**Key implementation notes:**
- Do not add `db_path`, `artifact_dir`, or any other field from `ANVILML_DESIGN.md
  §15`'s table yet — each field is added by the task that introduces the matching
  `ServerConfig` struct field, in Phase 2, to keep this file from drifting ahead of
  the actual config schema it's supposed to mirror.

**Acceptance criterion:**
```bash
cat anvilml.toml
# -> shows exactly: host, port (no other keys)
```

---

### Group D — Health endpoint

#### P1-D1: GET /health handler returns 200 OK

**Goal:** Wire the first real HTTP route through the full stack — handler, router,
and the binary's own serve loop — establishing the routing and serve pattern every
later handler reuses.

**Files to create or modify:**
- `crates/anvilml-server/src/handlers/mod.rs` — declares the `health` submodule.
- `crates/anvilml-server/src/handlers/health.rs` — `async fn health() -> StatusCode`
  returning `StatusCode::OK`.
- `crates/anvilml-server/src/lib.rs` — `pub fn build_router() -> axum::Router`
  registering `GET /health`.
- `backend/src/main.rs` — binds a `TcpListener` on `cli.host:cli.port`, calls
  `axum::serve(...)` raced via `tokio::select!` against `wait_for_shutdown_signal()`.

**Key implementation notes:**
- `anvilml-server/src/lib.rs` must stay within the 80-line cap — it holds only
  `build_router()` and `AppState`-related re-exports as they're introduced in later
  phases, never handler logic itself.
- The `tokio::select!` race between the server future and the shutdown signal is the
  first piece of the eventual full graceful-shutdown sequence; only the "stop
  accepting connections and exit" half exists at this phase — the 30-second worker
  drain (`ANVILML_DESIGN.md §19.3` steps 2–5) has no workers to drain yet.

**Acceptance criterion:**
```bash
cargo test -p anvilml-server --test health_tests
# -> >=1 test, exits 0
```

#### P1-D2: Runnable Proof: live binary answers /health over real TCP

**Goal:** Produce this phase's Runnable Proof — confirming the built binary, not just
the test suite, answers a real HTTP request — and record the transcript.

**Files to create or modify:**
- None. This task runs the already-built binary; see Acceptance Criterion.

**Key implementation notes:**
- `main.rs`'s CLI defaults (`P1-A2`) already match `anvilml.toml`'s values (`P1-C1`);
  no config-loading code exists yet to wire the file in, so no behavior change is
  required or introduced by this task — it is a verification step only.
- Record the literal terminal output (the `200` response) in the implementation
  report; this is what `docs/RUNNABLE_PROOF.md`'s Phase 1 entry will reference.

**Acceptance criterion:**
```bash
cargo build --release -p anvilml
./target/release/anvilml &
sleep 1
curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8488/health
# -> 200
kill %1
```

---

### Group E — CI

#### P1-E1: CI: ci.yml rust-test matrix job (real commands)

**Goal:** Create the CI workflow file with a real, working `rust-test` job that
exercises the now-buildable workspace across both target operating systems via a
single matrix-driven job body.

**Files to create or modify:**
- `.github/workflows/ci.yml` — new file; one job, `rust-test`, with
  `strategy.matrix.os: [ubuntu-latest, windows-latest]`.

**Key implementation notes:**
- Steps, in order: checkout; install Rust (toolchain comes from
  `rust-toolchain.toml`); a step gated `if: matrix.os == 'ubuntu-latest'` running
  `cargo fmt --all -- --check` (Linux-only, per `ENVIRONMENT.md §6` Step 11 and CI
  §18.3); `cargo clippy --workspace --features mock-hardware -- -D warnings`; `cargo
  test --workspace --features mock-hardware`.
- This must be one job with a matrix axis, not two near-duplicate job blocks — the
  project owner's explicit instruction for this phase.

**Acceptance criterion:**
```bash
cargo fmt --all -- --check && cargo clippy --workspace --features mock-hardware -- -D warnings && cargo test --workspace --features mock-hardware
# -> all exit 0 locally
# A pushed commit shows the rust-test job green on both matrix entries.
```

#### P1-E2: CI: ci.yml worker-test matrix + drift job placeholders

**Goal:** Complete the CI workflow file with the remaining jobs from the full 8-entry
matrix design, as placeholders for subsystems (`worker/`, OpenAPI generation, config
drift checking) that don't exist as real, exercisable code yet.

**Files to create or modify:**
- `.github/workflows/ci.yml` — adds `worker-test` (matrix: 4 entries — `{os, mode}` ×
  `{ubuntu-latest, windows-latest} × {mock, real}`), `openapi-drift`, and
  `config-drift` jobs.

**Key implementation notes:**
- `worker-test` is one matrix-driven job body (4 `include` entries), not 4 separate
  jobs. Its current step is a placeholder echo naming the mode
  (`echo "worker tests: no worker/ source yet (mode=${{ matrix.mode }})"`) — it
  becomes a real `pytest` invocation once Phase 7 (Real Worker Startup) lands.
- `openapi-drift` and `config-drift` are single jobs (no matrix needed yet) with the
  same placeholder-echo pattern, becoming real once `anvilml-openapi` emits a real
  spec and `ServerConfig` exists, respectively.

**Acceptance criterion:**
```bash
grep -c 'runs-on' .github/workflows/ci.yml
# -> 4 (rust-test, worker-test, openapi-drift, config-drift)
# A pushed commit shows all 4 jobs green.
```

---

## Phase Acceptance Criteria

```bash
# Standard gates
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test --workspace --features mock-hardware

# Runnable Proof (manual):
cargo build --release -p anvilml
./target/release/anvilml &
sleep 1
curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8488/health
# -> 200
kill %1
```

---

## Known Constraints and Gotchas

- Cargo workspace `members` cannot list a path that doesn't exist on disk yet — crate
  stub tasks (P1-B1…P1-B6) must add their own path to root `Cargo.toml` in the same
  task that creates the crate directory, in dependency order, never in a batch
  up-front.
- `cargo fmt --all -- --check` is Linux-only by existing project convention — do not
  add it to the Windows matrix leg.
- `anvilml-registry` and `anvilml-artifacts` intentionally have no `sqlx` dependency
  yet; do not add it speculatively in this phase.
- The `worker-test`, `openapi-drift`, and `config-drift` CI jobs are placeholders by
  design in this phase — a green check mark on them at this stage proves only that
  the placeholder echo ran, not that any real subsystem passed.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 1 — Repository Scaffold

**Capability proved:** The built `anvilml` binary starts, binds an HTTP port, and
answers a real `GET /health` request with `200`.

\`\`\`bash
# Runnable Proof (manual):
cargo build --release -p anvilml
./target/release/anvilml &
sleep 1
curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:8488/health
# -> 200
kill %1
\`\`\`
```
