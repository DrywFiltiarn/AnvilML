# Tasks: Phase 023 — Documentation Site

| Field | Value |
|-------|-------|
| Phase | 023 |
| Name | Documentation Site |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 22 |

## Overview

Phase 023 builds the mdBook documentation site covering all operational aspects of AnvilML. The site lives under `docs-site/` and is generated into `docs-site/book/`. It is structured for hosting on GitHub Pages but requires no CI configuration change — the `mdbook build` command is the sole gate.

The documentation is split into three tasks along natural reader-journey lines: setup and introduction (P23-A1), the core operational chapters that most users need (P23-A2), and the reference chapters covering the node system, operations, and troubleshooting (P23-A3). All three tasks share a single `SUMMARY.md` defined in P23-A1; P23-A2 and P23-A3 add files that are already listed in that summary.

Every API endpoint example in the documentation must match the actual implemented routes. The simplest way to verify this is to run the server locally and execute the example `curl` commands before writing them into the docs.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | docs-site | P23-A1 … P23-A3 | mdBook setup + Introduction; core chapters; reference chapters |

## Prerequisites

Phase 022 complete: release packaging working (`P22-A1`). All REST endpoints and WebSocket events are implemented and stable. `ANVILML_DESIGN.md Appendix B` provides the canonical example workflow JSON.

## Task Descriptions

### Group A — docs-site

#### P23-A1: docs-site setup, SUMMARY.md, and Introduction chapter

**Goal:** Initialise the mdBook project and define the full table of contents. All chapter files referenced in `SUMMARY.md` must exist (even if empty stubs) so that `mdbook build` exits 0 after this task. Write the Introduction chapter.

**This task's JSON `defers_to` field is `["P23-A2", "P23-A3"]`** — the stub chapter files it creates (everything except Introduction) are intentionally empty, with their content delivered by those two tasks. Per `FORGE_TASK_AUTHORING_SPEC.md §12a`, this is recorded structurally in `defers_to`, not only in the prose below, and the stub site (`SUMMARY.md`, near the stubbed entries) must carry the matching code comment `<!-- defers_to: P23-A2, P23-A3 -->` per `FORGE_AGENT_RULES.md §9.7`.

**Files to create or modify:**
- `docs-site/book.toml` — new file; `[book]` with `title = "AnvilML"`, `src = "src"`
- `docs-site/src/SUMMARY.md` — new file; complete chapter listing: Introduction, Quickstart, Configuration, Models, API, Nodes, Operations, Troubleshooting
- `docs-site/src/introduction.md` — new file; what AnvilML is, who it is for, high-level architecture diagram in ASCII or Mermaid

**Key implementation notes:**
- `SUMMARY.md` must list all chapters; stub `.md` files for chapters written in P23-A2 and P23-A3 to prevent broken-link errors — each stub is a single `# <Chapter Title>` heading and nothing else, per this task's `defers_to`
- `book.toml` must not include mdbook as a Cargo dependency — it is installed separately via `cargo install mdbook`
- Do not add `docs-site/` to the Cargo workspace

**Acceptance criterion:** `mdbook build docs-site` exits 0; no broken internal links reported.

---

#### P23-A2: Quickstart, Configuration, Models, and API chapters

**Goal:** Write the four chapters that cover the user journey from first install through API usage. Each chapter must be complete enough to follow end-to-end with only the documentation as a guide.

**Files to create or modify:**
- `docs-site/src/quickstart.md` — binary download, provisioning, start server, submit ZiT job via curl, view PNG artifact
- `docs-site/src/configuration.md` — all `ServerConfig` fields with defaults and descriptions, env var overrides, TOML precedence per `ENVIRONMENT.md §3–4`
- `docs-site/src/models.md` — directory layout convention, `ModelKind` values, `ModelDtype` values, rescan endpoint
- `docs-site/src/api.md` — every REST endpoint from `ANVILML_DESIGN.md §12.4` with method, path, request/response shapes; WebSocket event catalogue from `§12.6`; a complete curl + `websocat` example session

**Key implementation notes:**
- All `curl` examples must use paths that match the implemented routes exactly (verify locally)
- The example workflow JSON in `api.md` must be the `zit_fp8.json` from `docs/example_workflows/` — do not paraphrase or summarise it; include the full JSON in a fenced code block

**Acceptance criterion:** `mdbook build docs-site` exits 0; all four chapter files are non-empty and complete.

---

#### P23-A3: Nodes, Operations, and Troubleshooting chapters

**Goal:** Write the three reference chapters. The Nodes chapter covers the dynamic node system for developers writing custom nodes. The Operations chapter covers day-to-day server management. The Troubleshooting chapter covers the most common failure modes.

**Files to create or modify:**
- `docs-site/src/nodes.md` — generic node philosophy (architecture-agnostic dispatch), slot type table from `ANVILML_DESIGN.md §10.2`, full baseline node table (9 nodes with INPUT_SLOTS and OUTPUT_SLOTS from `§10.3`), example workflow JSON (Appendix B)
- `docs-site/src/operations.md` — graceful shutdown procedure, ghost-reset procedure, crash recovery observable sequence, database backup, upgrade path
- `docs-site/src/troubleshooting.md` — preflight failures and remediation, provisioning stuck, CUDA OOM, port conflict, Windows-specific notes (PSScriptAnalyzer, PowerShell execution policy)

**Key implementation notes:**
- The baseline node table in `nodes.md` must exactly match `ANVILML_DESIGN.md §10.3` — do not invent slot names; copy from the design document
- `troubleshooting.md` must include the exact log message that appears for each failure mode so users can grep for it

**Acceptance criterion:** `mdbook build docs-site` exits 0; `mdbook test docs-site` exits 0; all internal links resolve; no chapter stubs remain.

---

## Phase Acceptance Criteria

```bash
mdbook build docs-site
mdbook test docs-site
```

## Known Constraints and Gotchas

- Use `mdbook = "0.4"` installed via `cargo install mdbook`. Do not include mdbook in the project's `Cargo.toml`.
- All API endpoint examples must match the actual implemented routes. Run the server locally to verify example curl commands before documenting them.
- `mdbook test` will attempt to execute any Rust code blocks marked as runnable. Do not use runnable Rust code blocks in documentation chapters; use `rust,no_run` or plain `bash`/`json` fences instead.
