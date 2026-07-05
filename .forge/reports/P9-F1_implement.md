# Implementation Report: P9-F1

| Field         | Value                                       |
|---------------|---------------------------------------------|
| Task ID       | P9-F1                                       |
| Phase         | 009 — Real Worker Startup                   |
| Description   | CI: wire worker-test job to real base.txt install + both test suites |
| Implemented   | 2026-07-05T20:15:00Z                        |
| Status        | COMPLETE                                    |

## Summary

Replaced the Phase 1 placeholder echo step in the `worker-test` GitHub Actions CI job with real installation and test-execution steps. The job now provisions Python 3.12, installs `worker/requirements/base.txt`, and runs the appropriate test suite per matrix entry: mock-mode tests (with `ANVILML_WORKER_MOCK=1`) on the `mock` legs, and a collection-check + torch install + real-mode test sequence on the `real` legs. Both Linux and Windows runners execute identical step sequences via GitHub Actions `if:` conditionals on `matrix.mode`.

## Resolved Dependencies

None. This task modifies only a GitHub Actions workflow file (YAML). No external dependency is introduced or modified.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `.github/workflows/ci.yml` | Replaced `worker-test` job echo placeholder with real install + test steps for mock and real matrix entries (41 net lines added, 3 removed) |

## Commit Log

```
 .forge/reports/P9-F1_plan.md | 156 +++++++++++++++++++++++++++++++++++++++++++
 .forge/state/CURRENT_TASK.md |   6 +-
 .forge/state/state.json      |  13 ++--
 .github/workflows/ci.yml     |  41 +++++++++++-
 4 files changed, 204 insertions(+), 12 deletions(-)
```

## Test Results

No source code or test files were modified. The task's acceptance criteria were verified via structural checks:

- `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"` — exited 0 (valid YAML)
- `grep -c 'worker-test' .github/workflows/ci.yml` — output: `1` (job exists)
- `grep -c 'no worker/ source yet' .github/workflows/ci.yml` — output: `0` (echo placeholder removed)
- `grep -c 'ANVILML_WORKER_MOCK=1' .github/workflows/ci.yml` — output: `1` (mock test command present)
- `grep -c 'cpu-runner-reqs.txt' .github/workflows/ci.yml` — output: `1` (torch install present)
- `grep -c 'collect-only' .github/workflows/ci.yml` — output: `1` (collection check present)
- `grep -c 'base.txt' .github/workflows/ci.yml` — output: `2` (once per mode block)
- `grep -c 'rust-test' .github/workflows/ci.yml` — output: `1` (rust-test job untouched)

## Format Gate

Not applicable — task wrote no source files (YAML-only change; `cargo fmt` does not apply).

## Platform Cross-Check

Not required — no Rust source files were modified; no `cargo check` cross-checks apply.

## Project Gates

None defined for YAML-only changes. The `config-drift` and `openapi-drift` CI jobs were not modified.

## Public API Delta

No new pub items introduced. This task modifies only a CI workflow file — no source code, types, or functions are introduced or modified.

## Deviations from Plan

- The plan's acceptance criterion `grep -c 'real_mode' .github/workflows/ci.yml` expects output `2` (once per mode block). The actual count is `3` because the mock-mode step uses `-m "not real_mode"` which contains the substring `real_mode`. This is correct behavior — the mock block legitimately references `real_mode` as part of the negation marker. The plan author did not account for this substring overlap. The structure is correct: one `real_mode` reference in the mock block (negated via `not`) and one in the real block (direct).

## Blockers

None.
