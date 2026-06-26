# Tasks: Phase 27 — End-to-End Validation

**Phase:** 27
**Name:** End-to-End Validation
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 2, 3, 25, 26

---

## Overview

This phase is deliberately unlike every prior phase in this delivery: per
`ANVILML_DESIGN.md §2.2` and §20's roadmap, "End-to-End Validation" is the **only**
phase whose Runnable Proof is explicitly manual, explicitly real-GPU-only, and
explicitly excluded from CI. It is run by the project owner, on their own AMD RX
9070 (ROCm ≥ 7.2) hardware, against real production-size checkpoints — never by the
Forge agent, never in GitHub CI, and never as something any task may be "blocked"
on. This phase's two tasks reflect that constraint precisely: one produces the
checklist document the project owner will follow by hand, and the other audits the
existing CI configuration to confirm — rather than assume — that nothing in the
repository's automated test suite secretly requires real GPU hardware to pass.

This phase exists at this exact point, after the full MVP model matrix is proven on
CPU with fixtures (Phase 26), because real-GPU verification is meaningful only once
every architecture's CPU-with-fixture real path is already proven correct —
`ANVILML_DESIGN.md §17.3` is explicit that real-GPU-only verification "never excuses
skipping the CPU-with-fixture real path," meaning this phase is additive
confirmation on real silicon, not a substitute for everything Phases 19–26 already
proved on CPU.

At the start of this phase, no document exists telling the project owner exactly
what to run on real hardware, and the CI configuration has never been explicitly
audited for an accidental GPU dependency. At the end: `docs/E2E_VALIDATION.md` gives
the project owner an exact, model-matrix-complete checklist to run manually, and the
CI audit confirms — with the actual `grep` output reviewed line by line — that every
job in `ci.yml` runs correctly on a GPU-less runner.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Manual checklist | P27-A1 | The project-owner-facing real-GPU validation document |
| B | CI audit | P27-B1 | Confirms no CI job accidentally requires real GPU hardware |

---

## Prerequisites

The full MVP model matrix (ZiT, Flux 2 Klein 4B, Flux 2 Klein 9B) must produce real
artifacts on torch CPU with fixture checkpoints per Phase 26 (P26-D1) and Phase 24
(P24-F1).

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §2.2` | P27-A1, P27-B1 | The exact environment table — only the project owner's own hardware has real GPU access; neither the Forge agent nor GitHub CI does |
| `ANVILML_DESIGN.md §20` roadmap | P27-A1 | "The only phase whose Runnable Proof is explicitly manual and explicitly excluded from CI" |
| `ANVILML_DESIGN.md §17.3` | P27-A1 | Real-GPU verification never excuses skipping the CPU-with-fixture real path |
| `ANVILML_DESIGN.md §2.3` | P27-A1 | The complete model matrix the checklist must cover, one row per architecture |

---

## Task Descriptions

### Group A — Manual checklist

#### P27-A1: docs/E2E_VALIDATION.md: manual real-GPU checklist for all 3 model rows

**Goal:** Produce the exact checklist the project owner runs by hand on real
hardware — not an automated test, and never executed by any agent task.

**Files to create or modify:**
- `docs/E2E_VALIDATION.md` — new; the manual checklist.

**Key implementation notes:**
- The disclaimer at the top of the file is load-bearing, not boilerplate: this
  checklist is never run by the Forge agent, is excluded from CI per
  `ANVILML_DESIGN.md §2.2`, and no task is ever blocked waiting on it. If real-GPU
  verification is genuinely needed for some other purpose, the blocking task's
  report states so under a `Blockers` section, and the project owner runs this
  checklist manually, outside the agent workflow entirely.
- Covers all three model matrix rows from `ANVILML_DESIGN.md §2.3`: ZiT+Qwen3-4B,
  Flux2Klein-4B+Qwen3-4B, Flux2Klein-9B+Qwen3-8B.
- The final check in each row is an actual **human visual inspection** of the
  produced PNG — not a structural validity check, since no automated test in this
  project can assess whether an image "looks correct."

**Acceptance criterion:**
```bash
test -s docs/E2E_VALIDATION.md
# -> file exists, contains the disclaimer near its top
```

---

### Group B — CI audit

#### P27-B1: CI: confirm no workflow job attempts real-GPU execution

**Goal:** Audit the entire CI configuration, built up incrementally across every
prior phase, to confirm — rather than assume — that no job has accidentally come to
depend on real GPU hardware.

**Files to create or modify:**
- `.github/workflows/ci.yml` — audited; fixed only if a violation is found.

**Key implementation notes:**
- This is a **verification-only** task — no CI change is expected. If the audit
  does find a violation, that's a defect introduced by an earlier phase, not
  something newly created here, and it must be fixed and documented as a finding in
  this task's report.
- The literal `grep` output for GPU/CUDA/ROCm-related strings is reviewed line by
  line in the report — each match confirmed to be a comment, a torch-CPU-wheel
  install step, or a mock-mode environment variable, never an actual GPU
  requirement.

**Acceptance criterion:**
```bash
grep -i 'gpu\|cuda\|rocm' .github/workflows/ci.yml
# -> every match reviewed and confirmed non-GPU-requiring
```

---

## Phase Acceptance Criteria

```bash
# Standard automated gates (unaffected by this phase's manual-only proof):
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test --workspace --features mock-hardware
ANVILML_WORKER_MOCK=1 python -m pytest worker/tests -v
python -m pytest worker/tests -v -m real_mode

# Runnable Proof: explicitly manual, explicitly excluded from CI, per
# ANVILML_DESIGN.md §2.2 and §20. There is no automated command for this phase's
# actual validation — see docs/E2E_VALIDATION.md, run by the project owner on
# real GPU hardware, outside the agent workflow and outside CI entirely.
```

---

## Known Constraints and Gotchas

- **No agent task, in this phase or any other, ever runs `docs/E2E_VALIDATION.md`'s
  checklist.** It exists solely for the project owner's own manual use on real
  hardware they personally have access to.
- A task elsewhere in the project that finds itself wanting to "verify" something
  on real GPU hardware should write a `Blockers` note and stop — never attempt to
  simulate, mock around, or skip the requirement silently.
- This phase's CI audit (`P27-B1`) is intentionally suspicious of the existing
  configuration rather than trusting that it's already correct — every prior
  phase's CI changes are reviewed here, not assumed clean by virtue of having
  passed code review when they were written.
- Real-GPU verification, once it happens, is **additive** confirmation — it never
  substitutes for the CPU-with-fixture real-mode tests every architecture phase
  (19–26) already required and passed.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 27 — End-to-End Validation

**Capability proved:** Not applicable in the automated sense — this phase's actual
validation is explicitly manual, real-GPU-only, and excluded from CI per
`ANVILML_DESIGN.md §2.2`. `docs/E2E_VALIDATION.md` gives the project owner an exact
checklist to run by hand on their own hardware, covering all three rows of the MVP
model matrix. This phase's automated deliverable is the CI audit (`P27-B1`)
confirming no existing job accidentally requires real GPU hardware.
```
