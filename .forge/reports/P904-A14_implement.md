# Implementation Report: P904-A14

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P904-A14                           |
| Phase         | 904 — Retrofit fixes               |
| Description   | worker/nodes/loader.py: LoadVae missing device arg (TypeError on first real call); LoadClip stale docstring |
| Implemented   | 2026-06-24T14:10:00Z              |
| Status        | COMPLETE                           |

## Summary

Fixed two defects in `worker/nodes/loader.py`: (1) `LoadVae.execute()` called `_load_vae_from_safetensors` with only 2 of 3 required positional arguments — `device` was missing, causing a `TypeError` on the first real-mode call. Added `self.ctx.device` as the third argument, matching the pattern used by `LoadModel.execute()` and `LoadClip.execute()`. (2) `LoadClip.execute()` docstring still contained a stale `Raises: NotImplementedError` section referencing a stub that was replaced in P904-A12. Updated the docstring to reflect the actual exceptions raised by the real dispatch path (`OSError` and `ValueError`).

## Resolved Dependencies

None. This task modifies only two lines of Python code — no dependency changes.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | worker/nodes/loader.py | Fix LoadVae.execute() call site (add device arg); update LoadClip.execute() Raises docstring |

## Commit Log

```
 .forge/reports/P904-A14_plan.md | 115 ++++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md    |   6 +--
 .forge/state/state.json         |  13 ++---
 worker/nodes/loader.py          |  12 +++--
 4 files changed, 134 insertions(+), 12 deletions(-)
```

## Test Results

```
cargo test --workspace --features mock-hardware
  - 170+ Rust tests: all passed, 0 failed

ANVILML_WORKER_MOCK=1 worker/.venv/bin/python -m pytest worker/tests/ -v
  - 98 Python tests: all passed, 0 failed
```

## Format Gate

```
cargo fmt --all -- --check
  - Exit 0, no formatting drift.
```

## Platform Cross-Check

```
cargo check --workspace --features mock-hardware
  - Finished dev [unoptimized + debuginfo] target(s)

cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu
  - Finished dev [unoptimized + debuginfo] target(s)

cargo check --bin anvilml
  - Finished dev [unoptimized + debuginfo] target(s)

cargo check --bin anvilml --target x86_64-pc-windows-gnu
  - Finished dev [unoptimized + debuginfo] target(s)
```

All four cross-checks exit 0.

## Project Gates

None applicable — this task modifies only a Python node file and does not:
- Add, rename, or remove a `ServerConfig` field (Gate 1 trigger)
- Modify handler signatures, utoipa annotations, or `AppState` fields (Gate 2 trigger)
- Add, remove, or rename a node type (Gate 3 trigger)

## Public API Delta

```
git diff HEAD -- worker/nodes/loader.py | grep '^+.*pub ' | head -40
  - (no output)
```

No new `pub` items introduced. The changes are:
- Internal call-site fix in `LoadVae.execute()` (private `_load_vae_from_safetensors` call)
- Docstring update in `LoadClip.execute()` (documentation only, no signature change)

## Deviations from Plan

None. The implementation matches the approved plan exactly:
1. `LoadVae.execute()` line 513: added `self.ctx.device` as third argument to `_load_vae_from_safetensors`, with an inline comment explaining the rationale (matching the pattern used in `LoadModel.execute()` lines 432-439).
2. `LoadClip.execute()` docstring lines 564-566: replaced stale `Raises: NotImplementedError` section with the accurate `OSError`/`ValueError` section matching other loader nodes' docstrings.

## Blockers

None.
