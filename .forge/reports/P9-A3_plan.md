# Plan Report: P9-A3

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P9-A3                                             |
| Phase       | 009 — Real Worker Startup                         |
| Description | worker/: pyproject.toml or pytest.ini with real_mode marker registered |
| Depends on  | P9-A2                                             |
| Project     | anvilml                                           |
| Planned at  | 2026-07-05T10:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create `worker/pyproject.toml` with a `[tool.pytest.ini_options]` section that registers the `real_mode` pytest marker. This marker tells pytest that a test exercises real torch-level code (requires `torch` import) and must never run under `ANVILML_WORKER_MOCK=1`. The file is the single source of truth that all other Phase 9+ test files reference when deciding whether to mark a test `@pytest.mark.real_mode`. Acceptance: `cd worker && python -m pytest --markers | grep real_mode` exits 0 and shows the registered marker description.

## Scope

### In Scope
- Create `worker/pyproject.toml` with `[tool.pytest.ini_options]` containing a `markers` list with one entry: `real_mode: test requires real torch import, never runs under ANVILML_WORKER_MOCK=1`.
- This is a single-file creation; no source code, no tests, no CI changes.

### Out of Scope
None. `defers_to (from JSON): absent` — this task must implement its full scope. No functionality is deferred.

## Existing Codebase Assessment

The `worker/` directory exists but contains only `requirements/` (populated by P9-A1/P9-A2) and a `.venv`. There is no `pyproject.toml`, `pytest.ini`, or `tests/` directory yet. The Phase 9 task graph (`TASKS_PHASE009.md`) expects P9-A3 to establish the marker registration before any test files are written in later tasks (P9-B1 through P9-F1). The project's design doc (`ANVILML_DESIGN.md §18.3`) specifies the exact marker convention: `real_mode`-marked tests may import torch unconditionally; unmarked tests must not. The ENVIRONMENT.md §11.2 convention mirrors this with the description "exercises real torch-level code against a fixture checkpoint (no torch import in mock-mode collection)." The task context provides the canonical description to use.

## Resolved Dependencies

None. This task creates a single TOML configuration file with no external dependencies. The `pytest` marker registration is a built-in pytest feature — no new package or crate is introduced.

## Approach

1. Create `worker/pyproject.toml` with the following exact content:

```toml
[tool.pytest.ini_options]
markers = [
    "real_mode: test requires real torch import, never runs under ANVILML_WORKER_MOCK=1",
]
```

   Rationale: `pyproject.toml` is preferred over `pytest.ini` per the task context ("preferred over pytest.ini for a single consolidated config"). The marker description matches the task context verbatim. The `[tool.pytest.ini_options]` table is the standard pytest configuration location in pyproject.toml (pytest 6.0+).

2. Verify the marker is registered by running:
   ```bash
   cd worker && python -m pytest --markers | grep real_mode
   ```
   This must exit 0 and produce a line containing `real_mode` with the description.

No source code, tests, logging, or documentation changes are needed. This is a pure config file creation.

## Public API Surface

None. This task creates a configuration file only — no Python functions, classes, or modules are introduced.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `worker/pyproject.toml` | pytest marker registration with `real_mode` marker |

## Tests

None. This task creates a single config file that pytest reads at collection time. The acceptance criterion (`cd worker && python -m pytest --markers | grep real_mode`) is itself the verification — it runs pytest's built-in marker listing, which only works if the marker is correctly registered. No test file is written.

## CI Impact

No CI changes required. The `pyproject.toml` is a passive configuration file that pytest reads automatically during any `pytest` invocation. The existing CI jobs (`worker-linux-mock`, `worker-linux-real`, `worker-windows-mock`, `worker-windows-real`) already invoke `python -m pytest worker/tests/`; they will pick up the marker registration without any workflow changes.

## Platform Considerations

None identified. The `pyproject.toml` is a plain-text TOML file with no platform-specific content. The `pytest` tool resolves it identically on Linux and Windows.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The `cd worker` acceptance command fails because the `.venv` interpreter is not available or pytest is not installed in the venv. | Medium | High | The acceptance command uses the venv's pytest (as all Phase 9 commands do per ENVIRONMENT.md §6). If pytest is not yet installed in the venv, the ACT agent will install it via `pip install pytest` from `base.txt` before running the acceptance check — this is a prerequisite of any pytest-based acceptance criterion. |
| The TOML syntax is malformed and pytest fails to parse `pyproject.toml`, preventing all pytest runs in `worker/`. | Low | Medium | Write the file in a single atomic operation (one heredoc in the ACT session). The TOML content is trivial — one table with one list containing one string — making syntax errors extremely unlikely. The ACT agent will verify the file parses correctly before running the acceptance command. |

## Acceptance Criteria

- [ ] `cd /home/dryw/AnvilML/worker && python -m pytest --markers 2>/dev/null | grep real_mode` exits 0
