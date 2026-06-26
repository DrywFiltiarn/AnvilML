# Tasks: Phase 30 — v4 Roadmap Closeout: Final Compliance Sweep

**Phase:** 30
**Name:** v4 Roadmap Closeout: Final Compliance Sweep
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** all (1–29)

---

## Overview

This phase is the v4 rewrite's final phase — not a new feature, but a project-wide
verification sweep confirming that the discipline every individual phase applied
locally to its own files actually held consistently across all 29 prior phases
combined. It re-runs the project's own mandatory checks (the `§9a.1`-style stub
sweep, `Gate 4`'s mock/real marker validation, the full standard gate suite, the
platform cross-check) at full project scope rather than per-task scope, and audits
the delivery's own living-index documents (`docs/PHASES.md`,
`docs/RUNNABLE_PROOF.md`) for internal consistency now that they're complete.

This phase exists because every individual phase's own closing checks were
necessarily scoped to that phase's own files — no phase before this one had reason
to re-verify, say, Phase 20's markers while working on Phase 28's auto-provisioning
logic. A 30-phase delivery is exactly the kind of scope where a small inconsistency
— a stale `defers_to` reference, a `RUNNABLE_PROOF.md` entry that drifted from its
source `TASKS_PHASE*.md` after an edit, a marker left unmarked in an edge case no
single phase's own tests happened to exercise — could exist without any individual
phase's own gate ever catching it. This phase is that catch-all, run once, at the
end, deliberately.

At the start of this phase, every individual phase's own gates have passed, but the
delivery as a whole has never been audited as a single unit. At the end: every test
file has a `docs/TESTS.md` entry; the project-wide stub/marker/`defers_to` sweeps
report zero unresolved findings; the complete standard gate suite and both platform
cross-checks pass against the full, final repository state; and the delivery's own
phase registry and runnable-proof log are confirmed internally consistent across all
30 phases.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Test catalogue | P30-A1 | `docs/TESTS.md` completeness audit and backfill |
| B | Stub/defers_to sweep | P30-B1 | Project-wide `§9a.1`-equivalent sweep |
| C | Marker sweep | P30-C1 | Project-wide `Gate 4` mock/real marker validation |
| D | Full gate suite | P30-D1 | The complete standard gate sequence, run once at full scope |
| E | Documentation audit | P30-E1 | `PHASES.md`/`RUNNABLE_PROOF.md` internal consistency |

---

## Prerequisites

All 29 prior phases must be complete, with each phase's own local gates having
passed at the time it closed.

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §17.1` | P30-A1 | `docs/TESTS.md`'s exact entry format |
| `FORGE_AGENT_RULES.md §9a.1` | P30-B1 | The stub-sweep documentation requirement, even when the result is empty |
| `ENVIRONMENT.md §8` Gate 4 | P30-C1 | The exact marker-validation commands |
| `ENVIRONMENT.md §6`–§7 | P30-D1 | The exact standard gate sequence and platform cross-check commands |

---

## Task Descriptions

### Group A — Test catalogue

#### P30-A1: docs/TESTS.md: audit completeness against every test file in the repo

**Goal:** Confirm `docs/TESTS.md`'s incremental, per-task maintenance actually
produced complete coverage, backfilling any gaps found.

**Files to create or modify:**
- `docs/TESTS.md` — backfilled if gaps are found; not restructured if already
  correct.

**Key implementation notes:**
- This is **verification-and-backfill**, not a rewrite — entries already correct
  from prior phases are left alone; only genuine omissions are added.

**Acceptance criterion:**
```bash
grep -rn 'fn test_\|#\[test\]\|def test_' crates/ backend/ worker/tests/
# -> every result cross-checked against a docs/TESTS.md entry; gaps backfilled
```

---

### Group B — Stub/defers_to sweep

#### P30-B1: Project-wide sweep: zero unmarked stubs, zero stale defers_to, zero TODOs

**Goal:** Run the project's stub-detection sweep at full project scope, not the
single-task scope every individual phase used.

**Files to create or modify:**
- Any file where a genuine finding requires a fix.

**Key implementation notes:**
- The three loader nodes' `NotImplementedError` placeholders (Phase 19) should all
  be closed by Phases 20, 22, 23 — finding any **remaining** one at this point is a
  genuine regression to fix, not an expected, already-flagged exception.

**Acceptance criterion:**
```bash
# Sweep commands and output (even if empty) recorded in the report per §9a.1
```

---

### Group C — Marker sweep

#### P30-C1: Project-wide sweep: every REAL_PATH_VERIFIED/MOCK_PATH_VERIFIED marker resolves

**Goal:** Re-run `Gate 4`'s marker validation across the entire `worker/nodes/`
tree, confirming no regression slipped through across ten-plus phases of
incremental node and arch-module work.

**Files to create or modify:**
- Any file where a genuine marker gap is found.

**Key implementation notes:**
- Each individual arch-module phase (20–26) already confirmed this property
  locally for its own files — this task's value is specifically in catching
  anything that didn't get caught by any single phase's narrower scope.

**Acceptance criterion:**
```bash
# Both Gate 4 grep -L commands return empty for every file in worker/nodes/
# defining a node or arch-module function
```

---

### Group D — Full gate suite

#### P30-D1: Project-wide sweep: full standard gate suite + both platform cross-checks

**Goal:** Run the complete standard gate sequence against the full, final
repository state — the project's final green-build confirmation.

**Files to create or modify:**
- None expected; this is a verification task.

**Key implementation notes:**
- This includes every gate this delivery has referenced throughout: format, lint
  (both feature configurations), the full test suite (Rust and Python, mock and
  real), both platform cross-checks, config-drift, openapi-drift, and the mdBook
  build.

**Acceptance criterion:**
```bash
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo clippy --bin anvilml -- -D warnings
cargo test --workspace --features mock-hardware
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
cargo check --bin anvilml
cargo check --bin anvilml --target x86_64-pc-windows-gnu
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests -v
python -m pytest worker/tests -v -m real_mode
cargo test -p anvilml --features mock-hardware -- config_reference
cargo run -p anvilml-openapi && git diff --exit-code api/openapi.json
mdbook build docs/book
# -> every command exits 0
```

---

### Group E — Documentation audit

#### P30-E1: docs/PHASES.md + RUNNABLE_PROOF.md: final consistency audit across all phases

**Goal:** Confirm the delivery's own living-index documents are internally
consistent now that all 30 phases exist.

**Files to create or modify:**
- `docs/PHASES.md`, `docs/RUNNABLE_PROOF.md` — fixed if any drift is found.

**Key implementation notes:**
- Specifically checks for drift between `RUNNABLE_PROOF.md`'s aggregated entries
  and each phase's own `TASKS_PHASE*.md` source — these were generated together but
  edited independently at various points across this delivery, and must not be
  assumed to have stayed in sync without checking.
- Every flagged Deviation across all 30 phases' task contexts must appear in
  `PHASES.md`'s amendments log or be otherwise documented as resolved.

**Acceptance criterion:**
```bash
# Cross-check performed and documented; any drift or missing log entry fixed
```

---

## Phase Acceptance Criteria

```bash
# This phase's acceptance criteria are its own five tasks' acceptance criteria,
# run cumulatively — see P30-D1 for the complete final gate sequence.
```

---

## Known Constraints and Gotchas

- This phase finds things, or it doesn't — both are valid outcomes, but **either
  way the finding (or its absence) must be explicitly documented**, per
  `FORGE_AGENT_RULES.md §9a.1`'s pattern of recording "0 findings" rather than
  silently skipping the sweep because it seemed unlikely to find anything.
- No genuine finding from this phase may be deferred to a hypothetical "Phase 31"
  — this is the delivery's last phase; a finding here is fixed here, or escalated
  as an explicit blocker for human review.
- This phase does not introduce new functionality — every task is verification,
  audit, or backfill against documentation/test-catalogue completeness. A task in
  this phase that finds itself writing substantial new production code has likely
  misunderstood its own scope.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 30 — v4 Roadmap Closeout: Final Compliance Sweep

**Capability proved:** The complete v4 delivery — all 29 prior phases combined —
passes every project-wide compliance check this project defines: the stub/marker
sweeps report their findings (zero, or fixed), the full standard gate suite and
both platform cross-checks pass against the final repository state, and the
delivery's own phase registry and runnable-proof log are confirmed internally
consistent.

\`\`\`bash
# See P30-D1 for the complete final gate sequence — every command exits 0.
\`\`\`
```
