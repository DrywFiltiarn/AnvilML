# Tasks: Phase 025 — Documentation Site

| Field | Value |
|-------|-------|
| Phase | 025 |
| Name | Documentation Site |
| Milestone group | Distribution readiness |
| Depends on phases | 1-24 |
| Task file | `forge/tasks/tasks_phase025.json` |
| Tasks | 12 |

## Overview

Phase 25 is written last, on purpose: the full user and operator manual is authored once, against the finished system, rather than dribbled across earlier phases where it would be partial and contradictory. It is an mdBook site (Markdown source, static HTML out, Rust-native tooling) hosted on GitHub Pages via an auto-deploy workflow. Each chapter is one task — Overview, Installation & First Run, Configuration Reference, Models, Using the API, How-tos, Troubleshooting, Operations, Release & Versioning, and Glossary/FAQ — sourced from the real behavior and the committed `anvilml.toml`/`ENVIRONMENT.md` so the configuration chapter cannot invent fields. This is the authoritative expanded form of the lean in-zip `QUICKSTART.md` from Phase 24 (Option A: the release zip stays self-contained with a short quickstart that points here). This is a one-time authoring pass with no drift-guard; mandated doc updates on future behavior changes are deliberately out of scope for the MVP and noted as future forge.py workflow work.

Every task in this phase implements **one module or one endpoint** plus its test. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the **Runnable Proof** below passes.

## Tasks

| Task | Module / File | Summary |
|------|-------------|---------|
| P25-A1 | `docs-site/book.toml + src/SUMMARY.md` | docs-site: mdBook scaffold (book.toml, SUMMARY.md, src tree) |
| P25-A2 | `docs-site/src/overview.md` | docs-site: Overview & Concepts chapter |
| P25-A3 | `docs-site/src/installation.md` | docs-site: Installation & First Run chapter |
| P25-A4 | `docs-site/src/configuration.md` | docs-site: Configuration Reference chapter |
| P25-A5 | `docs-site/src/models.md` | docs-site: Models chapter |
| P25-A6 | `docs-site/src/api.md` | docs-site: Using the API chapter (REST + WebSocket) |
| P25-A7 | `docs-site/src/howto.md` | docs-site: How-tos chapter |
| P25-A8 | `docs-site/src/troubleshooting.md` | docs-site: Troubleshooting chapter |
| P25-A9 | `docs-site/src/operations.md` | docs-site: Operations chapter |
| P25-A10 | `docs-site/src/release-versioning.md + glossary.md` | docs-site: Release & Versioning + Glossary/FAQ chapters |
| P25-A11 | `.github/workflows/docs.yml` | docs-site: GitHub Pages auto-deploy workflow |
| P25-A12 | `docs/PROOF_phase025.md` | docs-site: full-build proof and cross-links from README |

## Task details

#### P25-A1: docs-site: mdBook scaffold (book.toml, SUMMARY.md, src tree)

- **Prereqs:** P24-A8
- **Tags:** —

Create docs-site/ mdBook project: book.toml (title 'AnvilML Documentation', authors, git-repo-url, default theme, search enabled, build dir = book/). src/SUMMARY.md listing every chapter file created in this phase (Overview, Installation, Configuration, Models, Using the API, How-tos, Troubleshooting, Operations, Release & Versioning, Glossary/FAQ). Create empty src/*.md stubs referenced by SUMMARY so the book builds. Verify: mdbook build docs-site produces docs-site/book/index.html with working nav + search; mdbook test passes (no broken internal links).

#### P25-A2: docs-site: Overview & Concepts chapter

- **Prereqs:** P25-A1
- **Tags:** —

Write docs-site/src/overview.md: what AnvilML is (Rust backend engine that supervises Python inference workers, exposes REST+WebSocket, manages jobs/models/artifacts); the SindriStudio relationship (AnvilML is headless backend; SindriStudio is the separate launcher; BloomeryUI is the separate frontend); architecture at a glance (the 8 crates + Python worker), linking to ARCHITECTURE.md. Plain factual prose, no marketing. Verify: chapter renders in the book, links resolve, mdbook build clean.

#### P25-A3: docs-site: Installation & First Run chapter

- **Prereqs:** P25-A2
- **Tags:** —

Write docs-site/src/installation.md (the authoritative EXPANDED quickstart; the in-zip dist/QUICKSTART.md stays a lean pointer to this). Cover: download a release zip per platform; verify SHA256SUMS + GPG signature; extract; Linux chmod +x; run anvilml/anvilml.exe; first-run BACKGROUND auto-provisioning of the worker venv (needs Python 3.12 + internet, downloads torch/diffusers, API responsive immediately at :8488, progress via /v1/system/env + /v1/events provisioning.progress); how to confirm readiness; system requirements (CUDA/ROCm/CPU). Verify: renders, mdbook build clean, links resolve.

#### P25-A4: docs-site: Configuration Reference chapter

- **Prereqs:** P25-A3
- **Tags:** reasoning

Write docs-site/src/configuration.md from the committed anvilml.toml + docs/ENVIRONMENT.md (single source; do not invent fields). Document every ServerConfig field + nested section (host, port, db_path, artifact_dir, num_threads, num_interop_threads, venv_path, max_ipc_payload_mib, [[model_dirs]], [rocm], [frontend] modes local/remote/headless, [gpu_selection], [limits], [hardware_override]) with type, default, meaning, and the matching ANVILML_* env override + precedence (defaults < toml < env < CLI). Verify: every key present in anvilml.toml appears in this chapter; mdbook build clean.

#### P25-A5: docs-site: Models chapter

- **Prereqs:** P25-A4
- **Tags:** —

Write docs-site/src/models.md: the models/ directory layout with one subdir per ModelKind (diffusion, lora, vae, controlnet, clip, unet, upscale); supported file formats (.safetensors/.ckpt/.pt/.bin); how scanning + id derivation works at a high level; dtype inference from filename; how to trigger a rescan (POST /v1/models/rescan) and list models (GET /v1/models). Verify: renders, links to the API chapter resolve, mdbook build clean.

#### P25-A6: docs-site: Using the API chapter (REST + WebSocket)

- **Prereqs:** P25-A5
- **Tags:** reasoning

Write docs-site/src/api.md: overview of the REST surface (health, system, system/env, system/versions, jobs CRUD+cancel, models, workers, artifacts) and the /v1/events WebSocket (event names: job.queued/started/progress/image_ready/completed/failed/cancelled, worker.status, system.stats, provisioning.progress); a full worked example submitting a ZiT job via curl and watching events via websocat; pointer to the machine-readable openapi.json shipped in the release. Verify: renders, example commands accurate to the implemented endpoints, mdbook build clean.

#### P25-A7: docs-site: How-tos chapter

- **Prereqs:** P25-A6
- **Tags:** reasoning

Write docs-site/src/howto.md with task-oriented recipes: repair a corrupted/incomplete worker venv (delete venv dir -> restart -> background re-provision); change worker dependency versions (edit worker/requirements/<backend>.txt, recreate venv); switch GPU backend CUDA/ROCm/CPU (hardware_override or env); run headless (default) vs serving a CUSTOM frontend via frontend.mode (not BloomeryUI - SindriStudio runs that separately); cancel and delete jobs; find and read logs; check running versions (GET /v1/system/versions). Each: goal, steps, expected result. Verify: renders, mdbook build clean.

#### P25-A8: docs-site: Troubleshooting chapter

- **Prereqs:** P25-A7
- **Tags:** reasoning

Write docs-site/src/troubleshooting.md: symptom -> cause -> fix entries for preflight failures (python_missing, torch_unavailable), provisioning stuck/Failed (network, wrong Python, disk space), cuda_oom on a job (pipeline cache eviction, smaller model/res), worker crash + auto-respawn behavior (what's normal), port already in use (change --port), 503 responses (provisioning vs workers_unavailable), Windows-specific notes (line endings, venv path, SmartScreen on unsigned binary). Verify: renders, mdbook build clean.

#### P25-A9: docs-site: Operations chapter

- **Prereqs:** P25-A8
- **Tags:** —

Write docs-site/src/operations.md: graceful shutdown (Ctrl-C / SIGTERM drains workers + flushes WAL); backing up the SQLite DB (anvilml.db + -wal/-shm) and the artifacts/ dir; ghost-job reset behavior on restart; upgrading to a new release (replace binary + worker source, keep db/models/artifacts, re-run provisioning if requirements changed); log locations and rotation. Verify: renders, accurate to implemented behavior, mdbook build clean.

#### P25-A10: docs-site: Release & Versioning + Glossary/FAQ chapters

- **Prereqs:** P25-A9
- **Tags:** —

Write docs-site/src/release-versioning.md: how the workspace release version relates to per-crate versions; reading GET /v1/system/versions; the bump->auto-tag->signed-release flow (link docs/RELEASE.md); pre-release semantics (-suffix). Write docs-site/src/glossary.md: terms (job, graph, node, worker, artifact, pipeline, DAG, mock-hardware, provisioning) + a short FAQ. Verify: both render, links resolve, mdbook build clean.

#### P25-A11: docs-site: GitHub Pages auto-deploy workflow

- **Prereqs:** P25-A10
- **Tags:** reasoning

Create .github/workflows/docs.yml: on push to main affecting docs-site/** (and manual workflow_dispatch), install mdbook, run mdbook build docs-site, and deploy docs-site/book/ to GitHub Pages using the official actions/upload-pages-artifact + actions/deploy-pages with the pages permission/environment. Build must fail the job on broken links (mdbook test). Verify (documented): pushing a docs-site change publishes the updated site to the repo's GitHub Pages URL; a broken internal link fails CI.

#### P25-A12: docs-site: full-build proof and cross-links from README

- **Prereqs:** P25-A11
- **Tags:** —

Final assembly: ensure SUMMARY.md lists all chapters in reading order; add a top-level docs link in README.md (and dist/QUICKSTART.md pointer) to the published GitHub Pages URL; write docs/PROOF_phase025.md with the steps: mdbook build docs-site, open book/index.html, confirm every chapter renders with working nav + search, confirm no broken links (mdbook test). Complete when the full manual builds locally and a human can browse all chapters end to end.


## Runnable Proof

Build the manual locally and confirm every chapter renders, then confirm it publishes to GitHub Pages.

```bash
# Local build:
mdbook build docs-site
mdbook test docs-site          # fails on broken internal links
# open the generated site:
xdg-open docs-site/book/index.html   # (or open/start on macOS/Windows)
```

Expected locally: the book opens with a working table of contents and search; all ten chapters render; `mdbook test` reports no broken links. For hosting: a push to `main` touching `docs-site/**` runs `.github/workflows/docs.yml`, which builds the book and deploys it to the repository's GitHub Pages URL (and fails CI on a broken link). Phase done when the full manual builds clean locally, a human can browse every chapter end to end, and the Pages deploy workflow publishes the site.
