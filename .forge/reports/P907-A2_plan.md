# Plan Report: P907-A2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P907-A2                                     |
| Phase       | 907 — ZeroMQ IPC Transport                  |
| Description | Add pyzmq>=26.0 to worker/requirements/base.txt |
| Depends on  | none                                        |
| Project     | anvilml                                     |
| Planned at  | 2026-06-13T09:22:00Z                        |
| Attempt     | 1                                           |

## Objective

Add `pyzmq>=26.0` as a dependency in `worker/requirements/base.txt` so that the Python
worker has ZeroMQ bindings available for Phase 907's ZeroMQ IPC transport replacement.

## Scope

### In Scope
- Add a single line `pyzmq>=26.0` to `worker/requirements/base.txt`

### Out of Scope
- No changes to Rust code, Cargo.toml files, or any other dependency files
- No changes to worker Python source code
- No changes to tests, CI, or documentation
- No build, test, or install execution — acceptance is verified by the orchestrator

## Approach

1. Append `pyzmq>=26.0` as a new line at the end of `worker/requirements/base.txt`.
   - The version constraint `>=26.0` matches the Phase 907 prerequisite stated in
     `docs/TASKS_PHASE907.md` ("`pyzmq>=26.0` installable in the project venv before P907-A5 runs").
   - Verified via MCP (pypi-query): latest pyzmq is 27.1.0; 26.4.0 is the latest 26.x
     release; `>=26.0` is compatible with Python ≥3.8 (project uses 3.12).
2. No further file modifications.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `worker/requirements/base.txt` | Add `pyzmq>=26.0` line |

## Tests

None. This task modifies only a requirements text file; there is no source code or test
file to write or run.

## CI Impact

No CI changes required. The file is already part of the repository and is consumed by the
Python worker test gate (`ANVILML_WORKER_MOCK=1 python -m pytest worker/tests/ -v`),
which runs after `pip install -r worker/requirements/base.txt` during venv provisioning.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `pyzmq>=26.0` has a transitive dependency that conflicts with an existing base.txt dep | Low | High | pyzmq's sole runtime dependency is `cffi` (pypy only); no conflict expected with existing deps (diffusers, transformers, Pillow, msgpack, numpy, safetensors, pytest). |
| Build/provisioning script fails to install pyzmq on a specific platform (e.g. missing C compiler for wheel build) | Low | Medium | pyzmq ships pre-built wheels for Linux x86_64, macOS, and Windows on Python 3.8–3.13; fallback to source build requires `libzmq` dev headers which are standard on the build environment. |

## Acceptance Criteria

- [ ] `pip install -r worker/requirements/base.txt` exits 0 in the project venv
