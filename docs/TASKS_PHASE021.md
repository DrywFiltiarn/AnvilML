# Tasks: Phase 021 — Auto-Provisioning & Version Introspection

| Field | Value |
|-------|-------|
| Phase | 021 |
| Name | Auto-Provisioning & Version Introspection |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 20 |

## Overview

Phase 021 adds two operational quality-of-life features: automatic venv provisioning on first run, and the `GET /v1/system/versions` endpoint. After this phase the server is usable without manual provisioning steps, and clients can query all component versions programmatically.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | anvilml-core + server | P21-A1 … P21-A2 | ComponentVersions endpoint, auto-provision background task |

## Prerequisites

Phase 020 complete: all 6 CI jobs pass, test catalogue complete.

## Task Descriptions

### Group A

#### P21-A1: ComponentVersions type and GET /v1/system/versions

See context field.

#### P21-A2: Auto-provisioning background task on first run

See context field.

## Phase Acceptance Criteria

```bash
cargo test --workspace --features mock-hardware
curl http://127.0.0.1:8488/v1/system/versions
```

## Known Constraints and Gotchas

- The server must bind and serve `/health` immediately even while provisioning runs in the background. Job submissions return 503 until provisioning completes.
