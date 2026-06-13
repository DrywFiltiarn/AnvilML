# Tasks: Phase 000 — Repository Preamble

| Field | Value |
|-------|-------|
| Phase | 000 |
| Name | Repository Preamble |
| Project | anvilml |
| Status | Approved |
| Depends on phases | None. This is the first phase. |

## Overview

Phase 000 establishes the repository foundation before any Rust code is written. This phase exists to ensure every subsequent phase builds on a consistent, correctly-configured base: a pinned Rust toolchain, proper line-ending rules, a full Cargo workspace with crate skeletons, a GitHub Actions CI workflow structure, and the `.forge` directory layout the orchestrator requires.

Nothing in this phase is runnable as a server. The Runnable Proof is a compiler check and a toolchain version assertion — both of which must pass before Phase 001 writes any server logic.

The workspace declares all eight production crates plus the `backend` binary, all stubbed with empty `lib.rs` or `main.rs` files. Declaring them now means that every subsequent task can add dependencies and implementations without modifying the workspace manifest.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Repository files | P0-A1 | `.gitignore`, `.gitattributes`, `rust-toolchain.toml` |
| B | Cargo workspace | P0-B1 | Workspace `Cargo.toml` + all 9 crate skeletons |
| C | CI workflow | P0-C1 | GitHub Actions workflow stubs for all 6 CI jobs |
| D | Forge bootstrap | P0-D1 | `.forge/` directory structure + `state.json` seed |

## Prerequisites

None. This is the first phase.

## Task Descriptions

### Group A — Repository files

#### P0-A1: repo: .gitignore, .gitattributes, rust-toolchain.toml

**Goal:** Establish the three repository hygiene files that control what git tracks, line endings, and the pinned Rust toolchain version.

**Files to create:**
- `.gitignore` — ignore `target/`, `*.db`, `*.db-wal`, `*.db-shm`, `*.venv`, `worker/.venv`, `artifacts/`, `*.log`, `.DS_Store`
- `.gitattributes` — `*.sh text eol=lf`, `*.py text eol=lf`, `*.rs text eol=lf`, `*.md text eol=lf`, `*.ps1 text eol=crlf`, `*.toml text eol=lf`
- `rust-toolchain.toml` — `[toolchain] channel = "stable"` with `components = ["rustfmt", "clippy"]`

**Key implementation notes:**
- `rust-toolchain.toml` sets `channel = "stable"`, not a pinned version string, per `ANVILML_DESIGN.md §17.1`.
- `.gitattributes` must cover all file types present in the repository now and in the future.

**Acceptance criterion:** `rustc --version` prints the stable version; `git check-attr eol -- worker/ipc.py` reports `lf`.

### Group B — Cargo workspace

#### P0-B1: repo: Cargo workspace root + all 9 crate skeletons

**Goal:** Create the workspace `Cargo.toml` declaring all members, and stub each crate with a minimal `Cargo.toml` and empty source file. Confirm `cargo build --workspace --features mock-hardware` exits 0.

**Files to create:**
- Root `Cargo.toml` — `[workspace]` with `members`, `resolver = "2"`, `[workspace.package] version = "0.1.0"`, `[workspace.dependencies]` block pre-populated with `serde`, `serde_json`, `tokio`, `axum`, `tracing`, `zeromq`, `rmp-serde`, `sqlx`, `uuid`, `thiserror`, `tower-http`
- `backend/Cargo.toml` + `backend/src/main.rs` (stub `fn main() {}`)
- `crates/anvilml-{core,hardware,registry,ipc,worker,scheduler,server,openapi}/Cargo.toml` + `src/lib.rs` (stub `pub fn stub() {}`) or `src/main.rs` for openapi
- `anvilml-hardware/Cargo.toml` declares `[features] mock-hardware = []`; all dependents forward it

**Acceptance criterion:** `cargo build --workspace --features mock-hardware` exits 0 with zero warnings.

### Group C — CI workflow

#### P0-C1: repo: GitHub Actions CI workflow

**Goal:** Create `.github/workflows/ci.yml` with all 6 CI jobs matching `ENVIRONMENT.md §6 GitHub CI job matrix`.

**Files to create:**
- `.github/workflows/ci.yml` — jobs: `rust-linux` (ubuntu-latest: fmt check + clippy + test), `rust-windows` (windows-latest: clippy + test), `worker-linux` (ubuntu-latest: pytest), `worker-windows` (windows-latest: pytest), `openapi-drift` (ubuntu-latest: regenerate + diff), `config-drift` (ubuntu-latest: config_reference test). All jobs use `--features mock-hardware`. Python jobs set `ANVILML_WORKER_MOCK=1`.

**Acceptance criterion:** `cat .github/workflows/ci.yml | python3 -c "import sys,yaml; yaml.safe_load(sys.stdin)"` exits 0 (valid YAML); all 6 job names present.

### Group D — Forge bootstrap

#### P0-D1: repo: .forge directory and initial state

**Goal:** Create the `.forge/` directory layout the orchestrator reads and writes during sessions.

**Files to create:**
- `.forge/state/state.json` — `{"completed":[],"in_progress":null,"failed":[],"needs_review":[],"last_updated":""}`
- `.forge/state/CURRENT_TASK.md` — `Task: none\nStep: none\nStatus: COMPLETE\nUpdated: (timestamp)`
- `.forge/reports/.gitkeep`
- `.forge/tasks/.gitkeep` (tasks written by Forge as phases complete)

**Acceptance criterion:** `ls .forge/state/state.json` exits 0; JSON parses correctly.

## Phase Acceptance Criteria

```bash
rustc --version                                           # stable toolchain active
cargo build --workspace --features mock-hardware          # exits 0, zero warnings
cargo clippy --workspace --features mock-hardware -- -D warnings  # exits 0
```

## Known Constraints and Gotchas

- The `[workspace.dependencies]` block should pre-declare all major dependencies the project will use, so individual crates use `{ workspace = true }` throughout. This avoids version divergence across crates.
- `anvilml-openapi` is a binary crate (`src/main.rs`), not a library. Its Cargo.toml must declare `[[bin]] name = "anvilml-openapi"`.
- The `mock-hardware` feature must be forwarded through every crate that depends on `anvilml-hardware`. Check `ARCHITECTURE.md §5` for the forwarding rule.
