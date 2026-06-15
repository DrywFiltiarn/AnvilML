# Tasks: Phase 022 — Release Packaging

| Field | Value |
|-------|-------|
| Phase | 022 |
| Name | Release Packaging |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 21 |

## Overview

Phase 022 prepares the release distribution: a GitHub Actions workflow that builds and publishes signed release artifacts on tag push, and validated provisioning scripts for both platforms.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | release | P22-A1 … P22-A2 | Release workflow, provisioning scripts |

## Prerequisites

Phase 021 complete.

## Task Descriptions

### Group A

#### P22-A1: GitHub Release workflow

See context field.

#### P22-A2: Provisioning scripts validated

See context field.

## Phase Acceptance Criteria

```bash
cargo build --release
bash -n scripts/install_worker_deps.sh
```

## Known Constraints and Gotchas

- The release zip must contain `worker/` source so auto-provisioning works on first run. Python source is not embedded in the binary.
