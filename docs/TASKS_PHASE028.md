# Tasks: Phase 28 — Distribution

**Phase:** 28
**Name:** Distribution
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 2, 3, 18

---

## Overview

This phase implements the three concerns `ANVILML_DESIGN.md §20`'s "Distribution"
roadmap entry names: auto-provisioning (the server can bootstrap its own Python
venv on first run, rather than requiring the operator to run a provisioning script
manually first), version introspection (a real `--version` flag and a fully
populated `EnvReport`, replacing Phase 18's best-effort placeholder), and release
packaging (a documented, exact specification of what files constitute a shippable
release).

This phase exists at this point — after the full architecture matrix is proven
(Phases 19–26) and after End-to-End Validation's manual checklist exists (Phase
27) — because distribution concerns are orthogonal to correctness concerns: a
server that auto-provisions and reports its own versions correctly is no more
"correct" at generating images than one that doesn't, but it is what makes the
binary actually deployable by someone who isn't the project's own developer. This
phase deliberately stays scoped to exactly what `ANVILML_DESIGN.md`'s own
one-line roadmap entry specifies — it does not import scope from unrelated
planning documents describing a much larger, frontend-inclusive rebuild that is
explicitly out of AnvilML's boundary per `ANVILML_DESIGN.md §1`.

At the start of this phase, a missing venv causes every worker to report `Dead`
with no automatic remedy, and `EnvReport` is populated by a best-effort placeholder
from Phase 18. At the end: a missing venv is auto-provisioned at startup;
`EnvReport`'s fields are populated from real preflight checks; `anvilml --version`
reports accurate component versions even without a venv present; and
`docs/RELEASE.md` documents exactly what a release package contains and excludes.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Auto-provisioning | P28-A1 | Missing-venv bootstrap at startup |
| B | Version introspection (EnvReport) | P28-B1 | Real preflight checks replacing the Phase 18 placeholder |
| C | Version introspection (CLI) | P28-C1 | `--version` flag, works even with no venv |
| D | Release packaging | P28-D1 | `docs/RELEASE.md`'s exact package specification |
| E | Proof | P28-E1 | The phase's Runnable Proof |

---

## Prerequisites

`AppState`'s `env_report` field and its best-effort placeholder population must
exist per Phase 18 (P18-A1). `ComponentVersions` must exist per Phase 18 (P18-B2).

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §19.1` | P28-A1 | "let AnvilML auto-provision on startup" as an explicit alternative to manual provisioning |
| `ENVIRONMENT.md §5` | P28-B1 | The exact three preflight checks and their non-fatal/fatal handling |
| `ANVILML_DESIGN.md §18.7` | P28-D1 | Release build: single static binary, Python source not embedded |
| `ANVILML_DESIGN.md §1` | All tasks | AnvilML's scope boundary — headless backend only, no frontend/desktop packaging concerns |

---

## Task Descriptions

### Group A — Auto-provisioning

#### P28-A1: backend: startup auto-provisioning check + auto-invoke install scripts

**Goal:** Let the server bootstrap a missing Python venv automatically at
startup, rather than requiring the operator to run a provisioning script first.

**Files to create or modify:**
- `backend/src/main.rs` — adds the missing-venv check and auto-invoke logic.

**Key implementation notes:**
- This is a **missing-venv bootstrap only** — an existing venv is never
  re-provisioned on every startup, which would be slow and pointless.
- A provisioning failure logs `ERROR` and **startup continues anyway** — workers
  subsequently report `Dead`, which is the existing, correct failure mode from
  `ENVIRONMENT.md §5`, not a new one invented by this task.

**Acceptance criterion:**
```bash
cargo test -p anvilml --test auto_provision_tests
# -> >=5 tests, exits 0
```

---

### Group B — Version introspection (EnvReport)

#### P28-B1: anvilml-server: GET /v1/system/versions EnvReport population at startup

**Goal:** Replace Phase 18's best-effort `EnvReport` placeholder with the real
three-step preflight check sequence `ENVIRONMENT.md §5` specifies.

**Files to create or modify:**
- `backend/src/main.rs` — real preflight checks.

**Key implementation notes:**
- An unexpected Python version logs `WARN` but does **not** abort — non-fatal, per
  the doc's explicit handling.
- The torch-import check is a **lightweight subprocess probe**
  (`python -c "import torch; print(torch.__version__)"`), not a full worker spawn —
  faster, and a distinct concern from the actual worker startup sequence (Phase 9).

**Acceptance criterion:**
```bash
cargo test -p anvilml --test auto_provision_tests
# -> >=11 tests total in the file, exits 0
```

---

### Group C — Version introspection (CLI)

#### P28-C1: crates/anvilml-openapi or backend: --version CLI flag prints full versions

**Goal:** Expose component version introspection from the command line, working
correctly even in a degraded environment (no venv, no torch).

**Files to create or modify:**
- `backend/src/cli.rs` — adds `--version` (a flag, not a subcommand, following
  `clap`'s standard convention).

**Key implementation notes:**
- `--version` exits immediately after printing — no port binding, no `AppState`
  construction.
- Gracefully shows `None`/unavailable for `python_version`/`torch_version` when
  the venv is missing, rather than erroring — this flag must be useful precisely in
  the degraded state it's often invoked to diagnose.

**Acceptance criterion:**
```bash
cargo test -p anvilml --test version_tests
# -> >=4 tests, exits 0
```

---

### Group D — Release packaging

#### P28-D1: scripts/: release build verification + packaging checklist doc

**Goal:** Document exactly what constitutes a shippable release package, and
confirm the release build itself produces a single working binary.

**Files to create or modify:**
- `docs/RELEASE.md` — new; the packaging specification.

**Key implementation notes:**
- The binary does **not** embed Python source — `worker/` ships as a sibling
  directory, per `ANVILML_DESIGN.md §18.7`.
- Runtime-generated artifacts (`.venv`, `target/`, a local `anvilml.db`, `artifacts/`
  contents) are explicitly excluded from any release package — they're created on
  first run, never shipped pre-populated.

**Acceptance criterion:**
```bash
test -s docs/RELEASE.md
cargo build --release -p anvilml
# -> file exists; release build exits 0, produces a single binary
```

---

### Group E — Proof

#### P28-E1: Runnable Proof: fresh clone auto-provisions and reports versions correctly

**Goal:** Produce this phase's Runnable Proof, demonstrating the full
distribution story end to end on a real (if minimal) environment.

**Files to create or modify:**
- None. This task runs the already-built binary against a deliberately-removed
  venv; see Acceptance Criterion.

**Key implementation notes:**
- This is the first proof in the delivery that deliberately starts from a
  **missing** dependency (the venv) rather than an already-correct environment —
  confirming the auto-provisioning path, not just the happy path.

**Acceptance criterion:**
```bash
rm -rf worker/.venv
cargo build --release -p anvilml
timeout 120 ./target/release/anvilml &
sleep 90
kill %1
# -> process does not crash during auto-provisioning (generous timeout for the install)
./target/release/anvilml --version
# -> shows a real (non-None) python_version and torch_version
```

---

## Phase Acceptance Criteria

```bash
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test --workspace --features mock-hardware

# Runnable Proof (manual): see P28-E1 — a fresh clone with no venv auto-provisions
# at startup without crashing, and --version subsequently reports accurate,
# real component versions.
```

---

## Known Constraints and Gotchas

- Auto-provisioning never re-runs against an already-present venv — confirm the
  existence check, not just the install logic, is correctly gating this.
- A provisioning failure must never abort the server process — it logs `ERROR` and
  lets the existing `Dead`-worker/`503 workers_unavailable` failure mode handle the
  consequence, exactly as a manually-failed provisioning attempt already would.
- `--version` must remain useful in a degraded environment — this is precisely the
  scenario an operator invokes it to diagnose, so it must never itself fail just
  because the thing it's reporting on is missing.
- This phase's scope is strictly what `ANVILML_DESIGN.md §20`'s one-line
  "Distribution" entry specifies — auto-provisioning, version introspection,
  release packaging. It does not include any frontend, desktop app, installer GUI,
  or auto-update mechanism — those belong to a separate, unrelated planning
  document outside AnvilML's boundary per `ANVILML_DESIGN.md §1`.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 28 — Distribution

**Capability proved:** A fresh clone with no Python venv present auto-provisions
one at startup without crashing, and `anvilml --version` subsequently reports
accurate, real component versions (Rust, Python, torch) — the full distribution
story, end to end, on a deliberately degraded starting environment.

\`\`\`bash
# Runnable Proof (manual):
rm -rf worker/.venv
cargo build --release -p anvilml
timeout 120 ./target/release/anvilml &
sleep 90
kill %1
./target/release/anvilml --version
# -> shows a real (non-None) python_version and torch_version
\`\`\`
```
