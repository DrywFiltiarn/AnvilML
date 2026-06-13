# Tasks: Phase 023 — Documentation Site

| Field | Value |
|-------|-------|
| Phase | 023 |
| Name | Documentation Site |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 22 |

## Overview

Phase 023 builds the mdBook documentation site covering all operational aspects of AnvilML. The site is generated into `docs-site/book/` and can be hosted via GitHub Pages.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | docs-site | P23-A1 … P23-A3 | mdBook setup + all chapters |

## Prerequisites

Phase 022 complete: release packaging working.

## Task Descriptions

### Group A

See task context fields for chapter content.

## Phase Acceptance Criteria

```bash
mdbook build docs-site
mdbook test docs-site
```

## Known Constraints and Gotchas

- Use `mdbook = "0.4"` installed via `cargo install mdbook`. Do not include mdbook in the project's `Cargo.toml`.
- All API endpoint examples must match the actual implemented routes. Run the server locally to verify example curl commands before documenting them.
