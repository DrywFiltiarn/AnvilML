# Implementation Report: P9-A2

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P9-A2                              |
| Phase         | 009 — Real Worker Startup          |
| Description   | worker/requirements/: real torch CPU wheel pin in cpu-* files |
| Implemented   | 2026-07-05T11:40:00Z               |
| Status        | COMPLETE                           |

## Summary

Populated the two empty placeholder requirement files created by P9-A1 with an identical torch CPU wheel pin. Both `worker/requirements/cpu-linux-agent.txt` and `worker/requirements/cpu-runner-reqs.txt` now contain `--index-url https://download.pytorch.org/whl/cpu` followed by `torch==2.12.1`, ensuring only the CPU-only PyTorch build is resolved by pip in Forge agent CI and GitHub CI runner real-mode tests.

## Resolved Dependencies

| Type   | Name    | Version resolved | Source         |
|--------|---------|------------------|----------------|
| python | torch   | 2.12.1           | pypi-query MCP |

The PyPI MCP reports `latest_version: "2.12.1"`. Compatible with Python 3.10+ (task target is Python 3.12.x per ENVIRONMENT.md §1). The CPU-only build is served from PyTorch's custom index (`https://download.pytorch.org/whl/cpu`).

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/requirements/cpu-linux-agent.txt` | Torch CPU wheel pin for Forge agent CI worker-test job |
| CREATE | `worker/requirements/cpu-runner-reqs.txt` | Identical torch CPU wheel pin for GitHub CI runner real-mode tests |

## Commit Log

```
 .forge/state/CURRENT_TASK.md            |  6 +++---
 .forge/state/state.json                 | 13 +++++++------
 worker/requirements/cpu-linux-agent.txt |  3 +++
 worker/requirements/cpu-runner-reqs.txt |  3 +++
 4 files changed, 16 insertions(+), 9 deletions(-)
```

## Test Results

Not applicable — this task creates plain-text requirements files with no executable code. No test file is created or modified. The acceptance criteria are shell commands (`pip install --dry-run` and `cmp`), not test functions.

The full Rust test suite was run to confirm no regressions: **247 tests passed, 0 failed**.

## Format Gate

```
(cargo fmt --all -- --check produced no output — no formatting drift)
```

## Platform Cross-Check

Not required — task writes no source files. No `#[cfg(...)]` guards, platform-specific code, or binary artifacts are involved.

## Project Gates

None triggered — this task modifies plain-text requirements files only. No `ServerConfig` fields, handler signatures, node types, or parity markers are added/modified.

## Public API Delta

No new pub items introduced. This task writes plain-text requirements files only.

## Deviations from Plan

None. Implementation matches the approved plan exactly:
- Torch version 2.12.1 resolved via MCP and pinned in both files.
- Both files contain identical content (verified by `cmp`).
- `--index-url https://download.pytorch.org/whl/cpu` followed by empty line, then `torch==2.12.1`.
- No source code modified, so no version bump, no format changes, no lint issues.

## Blockers

None.
