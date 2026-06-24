# Implementation Report: P904-Z2

| Field         | Value                                                           |
|---------------|-----------------------------------------------------------------|
| Task ID       | P904-Z2                                                         |
| Phase         | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects)           |
| Description   | pytest.ini + .github/workflows/ci.yml: register realcpu marker, exclude from CI |
| Implemented   | 2026-06-24T19:10:00Z                                            |
| Status        | COMPLETE                                                        |

## Summary

Registered the `realcpu` pytest marker in `worker/tests/pytest.ini` and added the
`-m "not realcpu"` selection expression to the CI worker job's pytest invocation in
`.github/workflows/ci.yml`. These two config-only changes ensure that future real-mode
CPU tests decorated with `@pytest.mark.realcpu` are excluded from CI's default pytest
run, which operates in a `torch`-absent environment.

## Resolved Dependencies

None. This task modifies only configuration files (`pytest.ini`, `ci.yml`) — no new
dependencies, no external crate or package references.

| Type | Name | Version resolved | Source |
|------|------|-----------------|--------|
| (n/a) | (n/a) | (n/a) | (n/a) |

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| Modify | worker/tests/pytest.ini | Added `markers` section registering `realcpu` marker |
| Modify | .github/workflows/ci.yml | Added `-m "not realcpu"` to pytest invocation in worker job (line 80) |

## Commit Log

```
 .forge/state/CURRENT_TASK.md |  6 +++---
 .forge/state/state.json      | 13 +++++++------
 .github/workflows/ci.yml     |  2 +-
 worker/tests/pytest.ini      |  2 ++
 4 files changed, 13 insertions(+), 10 deletions(-)
```

## Test Results

All acceptance criteria verified via grep:

```bash
$ grep -q 'realcpu' worker/tests/pytest.ini && echo PASS || echo FAIL
PASS: realcpu marker found in pytest.ini

$ grep -q 'not realcpu' .github/workflows/ci.yml && echo PASS || echo FAIL
PASS: not realcpu found in ci.yml

$ grep -c '^markers' worker/tests/pytest.ini
1

$ python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml')); print('PASS')"
PASS: ci.yml is valid YAML
```

No new test functions were written or modified — this task modifies configuration only.

## Format Gate

Not applicable — task wrote no source files (no Rust `.rs`, no Python `.py` files).
The project formatter (`cargo fmt --all`) operates on Rust source files only.

## Platform Cross-Check

Not required — no secondary platform target defined in docs/ENVIRONMENT.md for config
files. Both `pytest.ini` and `.github/workflows/ci.yml` are platform-neutral.

## Project Gates

None defined — no Rust source files were modified, so no config surface sync, OpenAPI
drift, or node parity gates are triggered.

## Public API Delta

No new pub items introduced. This task modifies only configuration files — no Python
classes, functions, or Rust pub items are introduced or changed.

## Deviations from Plan

None. Implementation exactly matches the approved plan.

## Blockers

None.
