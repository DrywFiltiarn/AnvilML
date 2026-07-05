# Implementation Report: P9-A1

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P9-A1                              |
| Phase         | 9 — Real Worker Startup            |
| Description   | worker/: requirements/base.txt (no torch, core deps only) |
| Implemented   | 2026-07-05T11:20:00Z               |
| Status        | COMPLETE                           |

## Summary

Created and updated three requirements files in `worker/requirements/`. The core dependency manifest `base.txt` was rewritten with `==` pins at MCP-resolved versions (diffusers, msgpack, pillow, pyzmq, pytest, safetensors, transformers) in alphabetical order — no torch, no comments, exactly seven lines. The placeholder `cpu-linux-agent.txt` was created as an empty file (singular name per ANVILML_DESIGN.md §3.1). The existing `cpu-runner-reqs.txt` was cleared to zero bytes per the task's instruction to create it as a currently-empty placeholder. All workspace checks (compile, format, clippy, all four platform cross-checks) pass with zero warnings and zero errors.

## Resolved Dependencies

| Type   | Name        | Version resolved | Source         |
|--------|-------------|------------------|----------------|
| python | diffusers   | 0.39.0           | pypi-query MCP |
| python | msgpack     | 1.2.1            | pypi-query MCP |
| python | pillow      | 12.3.0           | pypi-query MCP |
| python | pyzmq       | 27.1.0           | pypi-query MCP |
| python | pytest      | 9.1.1            | pypi-query MCP |
| python | safetensors | 0.8.0            | pypi-query MCP |
| python | transformers| 5.13.0           | pypi-query MCP |

All seven versions were resolved via `pypi-query_get_package_info` and confirmed to match the approved plan's version table exactly.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | worker/requirements/base.txt | Rewritten with MCP-resolved `==` pins, alphabetical order, no torch |
| CREATE | worker/requirements/cpu-linux-agent.txt | Empty placeholder (0 bytes) per ANVILML_DESIGN.md §3.1 singular naming |
| MODIFY | worker/requirements/cpu-runner-reqs.txt | Cleared to empty placeholder (0 bytes); removed prior torch pins |

## Commit Log

```
 .forge/state/CURRENT_TASK.md            |  6 +++---
 .forge/state/state.json                 | 19 +++++++++++++------
 worker/requirements/base.txt            | 14 +++++++-------
 worker/requirements/cpu-runner-reqs.txt |  5 -----
 4 files changed, 23 insertions(+), 21 deletions(-)
```

(Note: `.forge/state/` changes were pre-existing from The Forge orchestrator; only `base.txt` and `cpu-runner-reqs.txt` changes were introduced by this task. `cpu-linux-agent.txt` is a new untracked file included in `git add -A`.)

## Test Results

Not applicable — this task writes only requirements/manifest files (no `.rs` or `.py` source files). No tests, no compile errors, no lint warnings.

## Format Gate

```
cargo fmt --all -- --check
```

Exit 0 — no formatting drift detected.

## Platform Cross-Check

```
# 1. Mock-hardware Linux
cargo check --workspace --features mock-hardware
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.43s

# 2. Mock-hardware Windows
cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.65s

# 3. Real-hardware Linux
cargo check --bin anvilml
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.54s

# 4. Real-hardware Windows
cargo check --bin anvilml --target x86_64-pc-windows-gnu
→ Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.69s
```

All four checks exit 0.

## Project Gates

None defined for this task. This task modifies only requirements files — it does not trigger any of the four project gates (config surface sync, OpenAPI drift, node parity, mock/real parity markers).

## Public API Delta

No new pub items introduced. This task modifies only requirements/manifest files.

## Deviations from Plan

None. All three file operations were implemented exactly as specified in the approved plan. MCP-resolved versions match the plan's version table exactly.

## Blockers

None.
