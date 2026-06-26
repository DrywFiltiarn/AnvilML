# Tasks: Phase 2 — Core Domain Types: Config & Errors

**Phase:** 2
**Name:** Core Domain Types: Config & Errors
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1

---

## Overview

This phase builds the two foundational pieces of `anvilml-core` that everything else
in the system reads from: the `AnvilError` enum (the single error type every fallible
operation in the codebase returns) and `ServerConfig` (the single source of runtime
configuration, loaded through the four-layer precedence chain defined in
`ANVILML_DESIGN.md §15`). Neither piece does any I/O of its own beyond reading a TOML
file and environment variables — `anvilml-core` remains a pure-data crate throughout
this phase.

This phase exists immediately after the scaffold because every later crate — hardware
detection, the registry, IPC, the worker pool, the scheduler, the server — returns or
consumes `AnvilError`, and every later subsystem reads its settings from
`ServerConfig`. Building these two types correctly now, with full layered-precedence
config loading and a config-drift test that proves the checked-in `anvilml.toml` never
quietly diverges from the compiled struct, prevents two entire classes of defect that
would otherwise surface awkwardly in much later phases.

At the start of this phase, `anvilml-core` is an empty stub crate (Phase 1). At the
end, it exports a complete `AnvilError` with an `IntoResponse` impl, a complete
`ServerConfig` with every field from the design doc, a `config_load::load()` function
implementing the full four-layer precedence chain, and `backend/main.rs` is wired to
actually use it instead of the Phase 1 CLI-only defaults. Phase 3 (the rest of
`anvilml-core`'s domain types — jobs, models, hardware, workers, nodes, events) and
Phase 4 (`anvilml-hardware`'s detectors) both depend on `AnvilError` existing exactly
as specified here.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Config & error types | P2-A1 … P2-A7 | `AnvilError`, `ServerConfig` (scalar + nested fields), the layered `config_load::load()`, `main.rs` wiring, and the `config_reference` drift test |

A single group is used for this phase: every task operates on the same two
tightly-coupled concerns (the error type and the config type) within one crate, and
splitting into multiple group letters would not improve navigability for a phase this
narrow in scope.

---

## Prerequisites

`anvilml-core` must exist and compile as an empty stub crate, per Phase 1's P1-B1.
`backend`'s binary must build and run with a CLI-flag-only configuration path, per
Phase 1's P1-A2/P1-D1, since P2-A6 replaces that path with `config_load::load()`.

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §5.2` | P2-A1 | Exact `AnvilError` variant list and the `IntoResponse` JSON body shape |
| `ANVILML_DESIGN.md §15` / `ENVIRONMENT.md §3–§4` | P2-A2, P2-A3, P2-A4, P2-A5 | Config field names, types, defaults, and the four-layer precedence order |
| `ENVIRONMENT.md §3` | P2-A5 | Exact `ANVILML_*` environment variable names and the `__` nested-field convention |

---

## Task Descriptions

### Group A — Config & error types

#### P2-A1: anvilml-core: AnvilError enum + IntoResponse impl

**Goal:** Produce the single error type every fallible function in the codebase will
return from this point forward, including its HTTP-response mapping, so later crates
never need to invent their own error type or guess at a status-code convention.

**Files to create or modify:**
- `crates/anvilml-core/src/error.rs` — the `AnvilError` enum and its
  `IntoResponse` impl.
- `crates/anvilml-core/src/lib.rs` — adds `mod error;` and `pub use error::AnvilError;`.
- `crates/anvilml-core/Cargo.toml` — adds `thiserror`, `axum`, `uuid`.

**Key implementation notes:**
- The variant list is fixed and exhaustive per `ANVILML_DESIGN.md §5.2` (as
  amended — see `docs/ADDENDUM_ARTIFACT_NOT_FOUND.md`) — `Db`, `Io`, `Serde`,
  `Ipc`, `PayloadTooLarge`, `WorkerNotFound`, `JobNotFound`, `InvalidGraph`,
  `CycleDetected`, `ModelNotFound`, `ArtifactNotFound`, `WorkersUnavailable`,
  `Internal`. Do not add or rename variants beyond this list; later phases
  reference these exact names.
- `ArtifactNotFound(String)` follows the same shape and `404` mapping as
  `WorkerNotFound`/`JobNotFound`/`ModelNotFound` — it exists from this task
  onward specifically so Phase 15's `GET /v1/artifacts/:hash` handler never needs
  a placeholder variant.
- The JSON response body shape is fixed: `{"error": "<kind>", "message": "<text>",
  "request_id": "<uuid>"}` — `request_id` is a freshly generated UUID per response,
  not threaded through from a request-scoped extension at this stage (that wiring
  happens once `SetRequestIdLayer` exists, in the HTTP server phase).
- Resolve `thiserror`'s current version via the crates.io registry tool before
  pinning it in `Cargo.toml`.

**Acceptance criterion:**
```bash
cargo test -p anvilml-core --test error_tests
# -> >=9 tests, exits 0
```

#### P2-A2: anvilml-core: ServerConfig top-level scalar fields

**Goal:** Define the scalar half of `ServerConfig` — the fields with no nested
structure — establishing the struct and its `Default` impl before the nested tables
are layered on in the next task.

**Files to create or modify:**
- `crates/anvilml-core/src/config.rs` — `ServerConfig` struct (scalar fields only)
  and its `Default` impl.
- `crates/anvilml-core/src/lib.rs` — adds `mod config; pub use config::ServerConfig;`.

**Key implementation notes:**
- Fields and defaults, exactly: `host: String` (`"127.0.0.1"`), `port: u16` (`8488`),
  `db_path: PathBuf` (`"./anvilml.db"`), `artifact_dir: PathBuf` (`"./artifacts"`),
  `venv_path: PathBuf` (`"./worker/.venv"`), `model_scan_depth: u32` (`2`),
  `max_ipc_payload_mib: u32` (`256`), `num_threads: Option<u32>` (`None`).
- Do not add placeholder fields for the nested tables (`model_dirs`,
  `gpu_selection`, `limits`, `rocm`, `hardware_override`) in this task — they are
  P2-A3's explicit scope; adding empty stand-ins here would just create rework.

**Acceptance criterion:**
```bash
cargo test -p anvilml-core --test config_tests
# -> >=4 tests, exits 0
```

#### P2-A3: anvilml-core: ServerConfig nested table structs

**Goal:** Complete `ServerConfig` by adding the five nested-table fields, finishing
the struct to the exact shape the config file format (`anvilml.toml`'s `[[table]]`
and `[table]` sections) expects.

**Files to create or modify:**
- `crates/anvilml-core/src/config.rs` — adds `model_dirs`, `gpu_selection`, `limits`,
  `rocm`, `hardware_override` fields and their nested struct definitions; extends
  `Default`.

**Key implementation notes:**
- `model_dirs: Vec<ModelDirConfig>` defaults to an empty vec; each entry has `path:
  PathBuf`, `recursive: bool`, `max_depth: Option<u32>`.
- `gpu_selection: GpuSelectionConfig { default_device: String }` defaults to
  `"auto"`. `limits: LimitsConfig { max_queued_jobs: u32 }` defaults to `100`.
- `rocm: Option<RocmConfig>` and `hardware_override: Option<HardwareOverrideConfig>`
  both default to `None` — these are optional sections per `ENVIRONMENT.md §4`.
- This task receives the scope explicitly deferred by P2-A2's `defers_to` — confirm
  P2-A2's struct already has the eight scalar fields before adding to it.

**Acceptance criterion:**
```bash
cargo test -p anvilml-core --test config_tests
# -> >=9 tests total in the file, exits 0
```

#### P2-A4: anvilml-core: config_load layered precedence (defaults+toml)

**Goal:** Implement the first two layers of the four-layer config precedence chain —
compiled defaults overridden by an optional TOML file — as the foundation the next
task's env/CLI layers build on.

**Files to create or modify:**
- `crates/anvilml-core/src/config_load.rs` — `pub fn load(toml_path: Option<&Path>)
  -> Result<ServerConfig, AnvilError>`.
- `crates/anvilml-core/Cargo.toml` — adds `toml`.
- `crates/anvilml-core/src/lib.rs` — adds `mod config_load; pub use
  config_load::load;` (or equivalent re-export).

**Key implementation notes:**
- Start from `ServerConfig::default()`; if a TOML file is found (at `toml_path` or
  the default `./anvilml.toml`), parse it and override only the fields present in the
  file — absent fields keep their default, never silently reset to a TOML-side zero
  value.
- This task's scope is strictly the defaults→TOML two layers. Environment variables
  and CLI flags are P2-A5's scope — do not implement them here even partially.

**Acceptance criterion:**
```bash
cargo test -p anvilml-core --test config_load_tests
# -> >=4 tests (missing file, partial override, malformed TOML errors, full round-trip), exits 0
```

#### P2-A5: anvilml-core: config_load env var + CLI flag layers

**Goal:** Complete `load()`'s precedence chain with the two highest-priority layers —
environment variables, then CLI overrides — finishing the contract `backend/main.rs`
will call in the next task.

**Files to create or modify:**
- `crates/anvilml-core/src/config_load.rs` — extends `load()`; adds a
  `CliOverrides` struct.

**Key implementation notes:**
- Env var scan happens after the TOML merge and before CLI overrides, per
  `ANVILML_DESIGN.md §15`'s four-layer order. Variable names and the nested-field
  `__` convention (e.g. `ANVILML_GPU_SELECTION__DEFAULT_DEVICE` maps to
  `gpu_selection.default_device`) come from `ENVIRONMENT.md §3` — read the exact
  list there rather than reconstructing it from the field names alone.
- `CliOverrides { host: Option<String>, port: Option<u16> }` is applied last and
  wins over everything else; `None` fields mean "no override," not "set to default."

**Acceptance criterion:**
```bash
cargo test -p anvilml-core --test config_load_tests
# -> >=9 tests total in the file, exits 0
```

#### P2-A6: backend: wire config_load::load() into main.rs

**Goal:** Replace `backend/main.rs`'s Phase 1 CLI-only host/port handling with a real
call into the now-complete `config_load::load()`, making `ServerConfig` the actual
source of truth the running binary uses.

**Files to create or modify:**
- `backend/src/main.rs` — calls `anvilml_core::config_load::load(...)`; exits 1 with
  the error printed if loading fails, before any socket bind.
- `backend/src/cli.rs` — changes `host`/`port` fields from hardcoded-default values
  to `Option<String>`/`Option<u16>` with no default, so the CLI layer can correctly
  distinguish "user passed --port" from "user didn't pass --port" when building
  `CliOverrides`.

**Key implementation notes:**
- This is tagged `breaking` because it changes `cli.rs`'s public field types (`host`/
  `port` go from non-`Option` with a default to `Option` with none) — any code
  outside this task that read those fields directly needs to be checked, though none
  is expected to exist yet at this point in the project.
- The bind address/port used by `axum::serve` (Phase 1's P1-D1) now comes from the
  loaded `ServerConfig`, not from `cli.host`/`cli.port` directly.

**Acceptance criterion:**
```bash
cargo test --workspace --features mock-hardware
# -> exit 0 (includes the pre-existing shutdown and health tests still passing)
```

#### P2-A7: config_reference test: anvilml.toml matches ServerConfig

**Goal:** Close the loop between the checked-in reference config file and the
compiled struct with an automated drift check, so the two can never silently diverge
in a later phase.

**Files to create or modify:**
- `anvilml.toml` (repo root) — expanded to include every `ServerConfig` field at its
  documented default (per `ENVIRONMENT.md §4`), including a commented-out example
  `[[model_dirs]]` entry.
- `backend/tests/config_reference.rs` — loads `anvilml.toml` via `config_load::load`
  and asserts every field equals `ServerConfig::default()`.

**Key implementation notes:**
- `num_threads` is omitted from `anvilml.toml` since its documented default is the
  absence of a value (`None` = num_cpus), not a literal TOML value to write.
- This test is the `config-drift` CI job's real implementation, replacing the
  placeholder echo from Phase 1's P1-E2.

**Acceptance criterion:**
```bash
cargo test -p anvilml --features mock-hardware -- config_reference
# -> exit 0
```

---

## Phase Acceptance Criteria

```bash
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test --workspace --features mock-hardware
cargo test -p anvilml --features mock-hardware -- config_reference

# Runnable Proof (manual):
cargo build --release -p anvilml
ANVILML_PORT=9999 ./target/release/anvilml &
sleep 1
curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:9999/health
# -> 200  (proves the env-var layer of config_load::load() actually took effect —
#          the binary bound port 9999, not the compiled default 8488)
kill %1
```

---

## Known Constraints and Gotchas

- `ServerConfig`'s nested tables (P2-A3) must not be stubbed early in P2-A2 — doing
  so would force P2-A3 to edit fields it didn't create rather than add new ones,
  contradicting the clean split between the two tasks.
- The env var scan in P2-A5 must run strictly after the TOML merge from P2-A4 and
  strictly before the CLI override — getting this order wrong silently inverts the
  precedence chain in a way no individual layer's own tests would catch in isolation.
- `P2-A6`'s `cli.rs` change is a breaking change to a public struct's field types;
  confirm no other task or file outside this phase already reads `cli.host`/
  `cli.port` as non-`Option` values before merging.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 2 — Core Domain Types: Config & Errors

**Capability proved:** The running `anvilml` binary actually loads its bind address
and port through the full layered config_load::load() chain — an environment
variable override changes observable runtime behaviour (which port the server binds).

\`\`\`bash
# Runnable Proof (manual):
cargo build --release -p anvilml
ANVILML_PORT=9999 ./target/release/anvilml &
sleep 1
curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:9999/health
# -> 200
kill %1
\`\`\`
```
