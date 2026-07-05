# Plan Report: P9-F1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P9-F1                                       |
| Phase       | 009 — Real Worker Startup                   |
| Description | CI: wire worker-test job to real base.txt install + both test suites |
| Depends on  | P9-E1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-07-05T18:00:00Z                        |
| Attempt     | 1                                           |

## Objective

Replace the Phase 1 placeholder echo in the `worker-test` GitHub Actions CI job with real installation and test-execution steps. The job currently has a 4-entry matrix (ubuntu-latest × {mock, real} and windows-latest × {mock, real}) that only prints a message. This task wires each leg to: install `worker/requirements/base.txt`, then run the appropriate test suite (mock-mode on the mock legs, real-mode on the real legs with the prescribed collection-check + torch install sequence). This makes CI actually validate the Python worker code written in earlier Phase 9 tasks.

## Scope

### In Scope
- Modify `.github/workflows/ci.yml`: replace the `worker-test` job's single echo step with real install and test steps for all 4 matrix entries.
- Mock legs (`mode: mock`): install `base.txt`, run `ANVILML_WORKER_MOCK=1 python -m pytest worker/tests -v -m "not real_mode"`.
- Real legs (`mode: real`): install `base.txt`, run a mock-suite collection check (`pytest --collect-only -m "not real_mode"`), install `cpu-runner-reqs.txt`, run `python -m pytest worker/tests -v -m real_mode`.
- No platform-specific branch logic — the same install order applies to both Linux and Windows.
- No changes to any other CI job, any source file, or any config file.

### Out of Scope
None. `defers_to (from JSON): []` — this task has an empty defers_to field and implements its full scope. No functionality is deferred.

## Existing Codebase Assessment

The `.github/workflows/ci.yml` file already defines the `worker-test` job with the correct 4-entry matrix structure (ubuntu/windows × mock/real). The only placeholder is a single `echo` step at lines 57–59. All prerequisite infrastructure exists: `worker/requirements/base.txt` contains the core dependencies (no torch), `worker/requirements/cpu-runner-reqs.txt` contains the torch CPU wheel pin, and `worker/pyproject.toml` registers the `real_mode` pytest marker. The worker source files (`worker_main.py`, `ipc.py`, `capability.py`, and test files in `worker/tests/`) are all in place from earlier Phase 9 tasks.

The established CI pattern in this project uses `actions/setup-python@v6` with `python-version: "3.12"`, and the `rust-test` job already provisions the worker venv via `scripts/install_worker_deps.sh` (Linux) and `.ps1` (Windows). The `worker-test` job needs its own Python provisioning and pip install steps — it currently shares a checkout step with `rust-test` but has no dependency installation.

No gap between the design doc and current source affects this approach: ANVILML_DESIGN.md §18.3 specifies the exact install order and test commands, and the required files all exist at the paths referenced.

## Resolved Dependencies

None. This task modifies only a GitHub Actions workflow file (YAML). It references existing project files (`worker/requirements/base.txt`, `worker/requirements/cpu-runner-reqs.txt`) and Python packages already pinned in those files. No new external dependency is introduced.

| Type   | Name | Version verified | MCP source | Feature flags confirmed |
|--------|------|-----------------|------------|------------------------|
| —      | —    | —               | —          | —                      |

## Approach

1. **Read the current `worker-test` job structure** (lines 41–59 of `.github/workflows/ci.yml`). The job already has the correct matrix definition and a single echo step. Confirm the matrix entries:
   - `ubuntu-latest` + `mock`
   - `ubuntu-latest` + `real`
   - `windows-latest` + `mock`
   - `windows-latest` + `real`

2. **Replace the echo step** with the real step sequence. The current step block is:
   ```yaml
   - name: Run worker tests
     run: |
       echo "worker tests: no worker/ source yet (mode=${{ matrix.mode }})"
   ```
   This must be replaced with a conditional step block that branches on `matrix.mode`:

   **For `mode: mock`** (both Linux and Windows):
   ```yaml
   - name: Set up Python
     uses: actions/setup-python@v6
     with:
       python-version: "3.12"

   - name: Install base dependencies
     run: pip install -r worker/requirements/base.txt

   - name: Run mock-mode tests
     run: ANVILML_WORKER_MOCK=1 python -m pytest worker/tests -v -m "not real_mode"
   ```

   **For `mode: real`** (both Linux and Windows):
   ```yaml
   - name: Set up Python
     uses: actions/setup-python@v6
     with:
       python-version: "3.12"

   - name: Install base dependencies
     run: pip install -r worker/requirements/base.txt

   - name: Check mock suite collection (no torch)
     run: python -m pytest worker/tests --collect-only -m "not real_mode"

   - name: Install torch CPU wheel
     run: pip install -r worker/requirements/cpu-runner-reqs.txt

   - name: Run real-mode tests
     run: python -m pytest worker/tests -v -m real_mode
   ```

3. **Use GitHub Actions conditional expressions** (`if:`) to select the correct step sequence per matrix entry. Use a single approach: two separate step blocks, one with `if: matrix.mode == 'mock'` and one with `if: matrix.mode == 'real'`. This avoids nested conditionals and is the simplest correct approach.

4. **Verify the resulting YAML** by checking that:
   - The `worker-test` job still exists with the same matrix.
   - Both step blocks are present (mock block + real block).
   - No other jobs in the workflow were modified.
   - The file is valid YAML (the `yaml` CLI tool or Python's `yaml.safe_load` can validate).

**Rationale on approach choice:** Using two separate conditional step blocks (rather than a single step with shell conditionals) is preferred because: (a) it makes each mode's steps independently visible in the GitHub Actions UI; (b) it avoids shell quoting issues with the `-m "not real_mode"` marker on Windows PowerShell vs. bash; (c) it follows the existing pattern in the `rust-test` job where platform-specific steps use `if:` guards.

## Public API Surface

None. This task modifies only a CI workflow file — no source code, no types, no functions are introduced or modified.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `.github/workflows/ci.yml` | Replace `worker-test` job echo placeholder with real install + test steps for mock and real matrix entries |

## Tests

This task modifies only a CI workflow file (YAML). No new source code or test files are introduced. The acceptance criterion is structural: the job exists with real (non-echo) steps, verifiable by grep and YAML parsing.

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `.github/workflows/ci.yml` | ci_yaml_valid | The workflow file is valid YAML after changes | `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"` exits 0 |
| `.github/workflows/ci.yml` | worker_test_job_exists | The `worker-test` job still exists in the workflow | `grep -c 'worker-test' .github/workflows/ci.yml` outputs `1` |
| `.github/workflows/ci.yml` | no_echo_placeholder | The echo placeholder step is removed | `grep -c 'no worker/ source yet' .github/workflows/ci.yml` outputs `0` |
| `.github/workflows/ci.yml` | mock_steps_present | Mock-mode steps (base install + mock pytest) are present | `grep -c 'ANVILML_WORKER_MOCK=1' .github/workflows/ci.yml` outputs `1` |
| `.github/workflows/ci.yml` | real_steps_present | Real-mode steps (collection check + torch install + real pytest) are present | `grep -c 'cpu-runner-reqs.txt' .github/workflows/ci.yml` outputs `1` |

## CI Impact

This task modifies the `worker-test` job in `.github/workflows/ci.yml`. The `rust-test` job, `openapi-drift` job, and `config-drift` job are untouched. The change adds 4 new actionable steps to the CI pipeline (2 per matrix entry × 2 modes), replacing 1 echo step. No new CI jobs are added; the existing 4-matrix structure is preserved.

## Platform Considerations

None identified. The install order and commands are identical on Linux and Windows — no `#[cfg(unix)]`-style branch logic is needed. The `pip install` command and `python -m pytest` invocation work the same on both platforms. The `-m "not real_mode"` marker is passed to pytest, which handles it internally; no shell-level parsing differences affect it.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `pip install -r worker/requirements/base.txt` may fail if a package version in base.txt is incompatible with GitHub Actions' Python 3.12 environment | Low | High | The base.txt file was created and verified in Phase 9 P9-A1; all versions were resolved via PyPI MCP at that time. If a new version constraint emerges, the ACT agent will update the pin with the MCP result. |
| `pip install -r worker/requirements/cpu-runner-reqs.txt` may fail on Windows if the torch CPU wheel index URL is inaccessible or the wheel is not available for Windows | Medium | High | The file already exists with `--index-url https://download.pytorch.org/whl/cpu` and `torch==2.12.1`. If the wheel is not available for Windows, the ACT agent must confirm at session start and add a platform-specific index URL if needed (documented as a stated reason for the branch). |
| `pytest --collect-only -m "not real_mode"` may fail if a mock-mode test file imports torch at module level (violating the marker convention) | Low | Medium | This is intentional — the collection check is designed to catch such violations. If it fails, the ACT agent must fix the offending test file (move the torch import behind the mock guard) as part of this task, since it blocks real-mode CI. |
| YAML indentation errors when replacing the echo step | Low | Medium | The ACT agent should validate the resulting YAML with `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"` before staging, as listed in acceptance criteria. |

## Acceptance Criteria

- [ ] `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"` exits 0
- [ ] `grep -c 'worker-test' .github/workflows/ci.yml` outputs `1`
- [ ] `grep -c 'no worker/ source yet' .github/workflows/ci.yml` outputs `0`
- [ ] `grep -c 'ANVILML_WORKER_MOCK=1' .github/workflows/ci.yml` outputs `1`
- [ ] `grep -c 'cpu-runner-reqs.txt' .github/workflows/ci.yml` outputs `1`
- [ ] `grep -c 'collect-only' .github/workflows/ci.yml` outputs `1`
- [ ] `grep -c 'mode.*mock' .github/workflows/ci.yml` outputs `1`
- [ ] `grep -c 'mode.*real' .github/workflows/ci.yml` outputs `1`
- [ ] `grep -c 'base.txt' .github/workflows/ci.yml` outputs `2` (once per mode block)
- [ ] `grep -c 'real_mode' .github/workflows/ci.yml` outputs `2` (once per mode block)
- [ ] `grep -c 'rust-test' .github/workflows/ci.yml` outputs `1` (rust-test job untouched)
