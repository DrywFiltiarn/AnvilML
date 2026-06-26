# Tasks: Phase 29 — Documentation

**Phase:** 29
**Name:** Documentation
**Project(s):** anvilml
**Status:** Draft
**Depends on phases:** 1, 2, 3, 10, 13, 14, 18, 28

---

## Overview

This phase builds the mdBook documentation site `ANVILML_DESIGN.md §20`'s
"Documentation" roadmap entry specifies: an mdBook scaffold, an API reference, and
a node SDK guide — each chapter sourced from content that already exists in the
project's other documents (`ENVIRONMENT.md`, `ANVILML_DESIGN.md`, `api/openapi.json`,
and the actual codebase) rather than independently re-derived prose that risks
drifting from the authoritative source. This phase's tasks are written with that
single concern in mind: every chapter must have exactly one source of truth, never
becoming a second, unsynchronized copy of something the project already maintains
correctly elsewhere.

This phase exists last, after Distribution (Phase 28), because the documentation it
produces describes the **finished** system — auto-provisioning, version
introspection, the complete REST/WebSocket surface, and the node SDK contract are
all things this phase documents as already-built, stable behavior, not aspirational
or in-progress features. This phase's scope is deliberately narrow: it covers
exactly what `ANVILML_DESIGN.md`'s own roadmap entry names (mdBook site, API
reference, node SDK guide) for AnvilML's own headless backend — it does not import
scope from any unrelated planning document describing frontend, extension,
or desktop-packaging concerns, which are explicitly outside AnvilML's boundary per
`ANVILML_DESIGN.md §1`.

At the start of this phase, no `docs/book/` directory exists. At the end: a
complete, internally-link-checked mdBook site exists with seven chapters
(Introduction, Getting Started, Configuration Reference, REST API Reference,
WebSocket Events, Node SDK Guide, Operations/Runbook), a new additive CI job
verifies the site builds cleanly on every push, and every chapter's content is
verified to stay synchronized with its actual authoritative source rather than
becoming a stale fourth copy of information the project already maintains.

---

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | Scaffold | P29-A1 | mdBook structure, `SUMMARY.md`, stub chapters |
| B | Getting Started & Config | P29-B1 | Sourced from `ENVIRONMENT.md` |
| C | REST API Reference | P29-C1 | Derived from the real `api/openapi.json`, never hand-transcribed separately |
| D | WebSocket Events | P29-D1 | Sourced from the actual `WsEvent` enum definitions |
| E | Node SDK Guide | P29-E1 | Sourced from the actual node system code, using `PassThrough` as the worked example |
| F | Operations/Runbook | P29-F1 | Consolidates existing scattered runbook content into one narrative |
| G | CI | P29-G1 | A new, additive `docs-build` job |

---

## Prerequisites

`api/openapi.json` must have real content per Phase 18 (P18-F1). `docs/RELEASE.md`
and the auto-provisioning/`--version` work must exist per Phase 28. The full node
system (`@register`/`BaseNode`/`SlotSpec`/`NodeContext`) must exist per Phase 10, and
`PassThrough` must exist per Phase 14 (P14-B1).

---

## Interfaces and Contracts

| Contract document | Relevant to tasks | What must match |
|--------------------|--------------------|------------------|
| `ANVILML_DESIGN.md §20` | P29-A1 | The exact Documentation roadmap scope — mdBook site, API reference, node SDK guide |
| `ANVILML_DESIGN.md §1` | P29-E1 | AnvilML's scope boundary — no frontend/extension SDK concept exists here |
| `FORGE_AGENT_RULES.md §5.5` | P29-G1 | "Preserve all existing jobs" — the new CI job is additive only |
| `ANVILML_DESIGN.md §10`, §10.4, §10.6 | P29-E1 | The node system specification this chapter documents |

---

## Task Descriptions

### Group A — Scaffold

#### P29-A1: docs/book/: mdBook scaffold + SUMMARY.md structure

**Goal:** Create the mdBook source directory and chapter structure, as scaffolding
only — no full chapter content yet.

**Files to create or modify:**
- `docs/book/book.toml`, `docs/book/src/SUMMARY.md` — new.
- Seven stub chapter files — title + one-sentence placeholder each.

**Key implementation notes:**
- The seven planned chapters mirror the project's existing documentation
  structure: Introduction, Getting Started, Configuration Reference, REST API
  Reference, WebSocket Events, Node SDK Guide, Operations/Runbook.
- This task explicitly does not write full chapter content — that's every
  subsequent task's scope.

**Acceptance criterion:**
```bash
mdbook build docs/book
# -> exits 0, no broken internal links, every SUMMARY.md entry resolves
```

---

### Group B — Getting Started & Config

#### P29-B1: docs/book/: Getting Started + Configuration Reference chapters

**Goal:** Write these two chapters by reusing `ENVIRONMENT.md`'s existing content,
reformatted for mdBook rendering — never independently re-derived prose.

**Files to create or modify:**
- `docs/book/src/getting-started.md`, `configuration-reference.md`.

**Key implementation notes:**
- Content is cross-checked against the **current** post-Phase-28 state of
  `ServerConfig`'s fields and auto-provisioning behavior — not an older draft.

**Acceptance criterion:**
```bash
mdbook build docs/book
# -> exits 0
```

---

### Group C — REST API Reference

#### P29-C1: docs/book/: REST API Reference chapter generated from api/openapi.json

**Goal:** Write the API reference chapter derived from the **real**
`api/openapi.json`, never as a separately hand-transcribed copy of the route table.

**Files to create or modify:**
- `docs/book/src/rest-api-reference.md` (or an embedded/linked spec asset).

**Key implementation notes:**
- Whichever approach is chosen (direct embed, or a generation step deriving the
  listing from the spec), it must not require hand-updating this chapter on every
  route change — `openapi-drift` (Phase 18's CI gate) already catches spec/handler
  drift, and this chapter must never become a third, unsynchronized copy of the
  same information.

**Acceptance criterion:**
```bash
mdbook build docs/book
# -> exits 0; the chapter's route listing verified against api/openapi.json's
# actual paths via a test or script, not a one-time manual check
```

---

### Group D — WebSocket Events

#### P29-D1: docs/book/: WebSocket Events chapter

**Goal:** Document the `GET /v1/events` connection sequence and every `WsEvent`
variant, sourced from the actual struct definitions.

**Files to create or modify:**
- `docs/book/src/websocket-events.md`.

**Key implementation notes:**
- Every example payload is sourced from the actual `WsEvent` enum (Phase 3's
  P3-A8/P3-A9) — not re-derived from memory of what the fields probably are.
- Includes the 1024-buffer-overflow disconnect rule from Phase 16.

**Acceptance criterion:**
```bash
mdbook build docs/book
# -> exits 0; every current WsEvent variant has a documented subsection,
# cross-checked against the actual enum, not assumed complete
```

---

### Group E — Node SDK Guide

#### P29-E1: docs/book/: Node SDK Guide chapter (BaseNode contract for new node authors)

**Goal:** Document the node system contract for someone writing a new node or arch
module, using the project's own simplest real node as the worked example.

**Files to create or modify:**
- `docs/book/src/node-sdk-guide.md`.

**Key implementation notes:**
- The worked example references the **actual** `PassThrough` node (Phase 14's
  P14-B1) — not an invented hypothetical, since `PassThrough` already exists and is
  simpler to point to than constructing a new pedagogical example.
- This guide covers AnvilML's own `worker/nodes/` system only — it is **not** an
  extension/plugin SDK for any frontend, since no such concept exists in AnvilML's
  scope per `ANVILML_DESIGN.md §1`.

**Acceptance criterion:**
```bash
mdbook build docs/book
# -> exits 0; the chapter's PassThrough reference matches the actual current
# file content, not a drifted paraphrase
```

---

### Group F — Operations/Runbook

#### P29-F1: docs/book/: Operations/Runbook chapter

**Goal:** Consolidate the operator-facing procedures currently scattered across
`ANVILML_DESIGN.md §19`, `ENVIRONMENT.md`, and `docs/RELEASE.md` into one coherent
narrative — not a fourth independent description of the same procedures.

**Files to create or modify:**
- `docs/book/src/operations-runbook.md`.

**Key implementation notes:**
- This is the **first** task in this phase where a build with stub-content chapters
  remaining would be a genuine regression — confirm zero stub placeholders remain
  anywhere in `docs/book/src/` after this task.

**Acceptance criterion:**
```bash
mdbook build docs/book
# -> exits 0, no broken links across all chapters now that every SUMMARY.md
# entry has real content
```

---

### Group G — CI

#### P29-G1: CI: add a docs-build job verifying mdBook builds cleanly

**Goal:** Add a new, additive CI job that verifies the documentation site
continues to build cleanly on every push, without touching any existing job.

**Files to create or modify:**
- `.github/workflows/ci.yml` — adds `docs-build`.

**Key implementation notes:**
- This is a strictly **additive** job — per `FORGE_AGENT_RULES.md §5.5`'s "preserve
  all existing jobs" rule, the existing `rust-test`/`worker-test`/`openapi-drift`/
  `config-drift` jobs are completely untouched.

**Acceptance criterion:**
```bash
grep -c 'docs-build' .github/workflows/ci.yml
# -> the new job exists
mdbook build docs/book
# -> exits 0 locally
```

---

## Phase Acceptance Criteria

```bash
cargo fmt --all -- --check
cargo clippy --workspace --features mock-hardware -- -D warnings
cargo test --workspace --features mock-hardware
mdbook build docs/book

# Runnable Proof: not applicable in the live-server sense — this phase's
# deliverable is the documentation site itself. `mdbook build docs/book` exiting
# 0 with no broken internal links, across all seven complete (non-stub) chapters,
# is the complete and sufficient proof of this phase's deliverable.
```

---

## Known Constraints and Gotchas

- **Every chapter must have exactly one source of truth.** The REST API Reference
  chapter must never become a second, hand-maintained copy of `api/openapi.json`'s
  route list; the WebSocket Events chapter must never drift from the actual
  `WsEvent` enum; the Node SDK Guide's worked example must reference the real
  `PassThrough` file, not a paraphrase that can silently go stale.
- The new `docs-build` CI job is strictly additive — it must never modify, disable,
  or reorder any existing job.
- This phase's scope is exactly what `ANVILML_DESIGN.md §20`'s "Documentation"
  roadmap entry names — it does not include an extension/plugin development guide,
  since AnvilML has no extension system and no frontend concern, per `§1`'s explicit
  scope boundary.
- No chapter is considered complete while it still contains a stub placeholder —
  `P29-F1` is the explicit checkpoint confirming zero stubs remain anywhere in
  `docs/book/src/`.

---

## docs/RUNNABLE_PROOF.md entry

```markdown
## Phase 29 — Documentation

**Capability proved:** A complete, seven-chapter mdBook documentation site builds
cleanly with no broken internal links, with every chapter's content sourced from
exactly one authoritative project source (no independently-drifting copies), and a
new additive CI job confirms this on every push.

\`\`\`bash
mdbook build docs/book
# -> exits 0, no broken internal links
\`\`\`
```
