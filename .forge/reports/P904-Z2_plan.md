# Plan Report: P904-Z2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P904-Z2                                     |
| Phase       | 904 — P18 D16–D20 Retrofit (Real-Path Wiring Defects) |
| Description | pytest.ini + .github/workflows/ci.yml: register realcpu marker, exclude from CI |
| Depends on  | P904-Z1b                                    |
| Project     | anvilml                                     |
| Planned at  | 2026-06-24T19:05:00Z                        |
| Attempt     | 1                                           |

## Objective

Register the `realcpu` pytest marker in `worker/tests/pytest.ini` so that tests annotated
with `@pytest.mark.realcpu` are excluded from CI's default pytest invocation, and update
`.github/workflows/ci.yml`'s worker job to explicitly pass `-m "not realcpu"` as a
defense-in-depth guard. This ensures the upcoming real-mode CPU test suite (P904-Z3, Z4,
Z5) is opt-in and never picked up by CI's `pytest worker/tests -v` run, which operates in
a `torch`-absent environment (base.txt deliberately excludes GPU-specific torch builds).

## Scope

### In Scope
- Update `worker/tests/pytest.ini`: add a `markers` line registering `realcpu` with a
  description matching the task context.
- Update `.github/workflows/ci.yml`: add `-m "not realcpu"` to the pytest invocation in
  the `worker` job's "Run worker tests" step (line 80).

### Out of Scope
None. This task has no deferrals (`defers_to: []` / absent). All described functionality
is implemented in full. Creating or modifying `worker/requirements/cpu-linux-agent.txt` is
out of scope (already exists on main). Creating the real-mode test files (test_real_loaders.py,
test_real_encoder_sampler.py, etc.) is handled by P904-Z3 and P904-Z4.

## Existing Codebase Assessment

The `worker/tests/pytest.ini` file exists with minimal content: a single `[pytest]` section
and `testpaths = tests`. There are no existing `markers` entries and no existing
`@pytest.mark` annotations in any test file under `worker/tests/` (confirmed by grep).

The CI workflow `.github/workflows/ci.yml` has a `worker` job that runs
`pytest worker/tests -v` on line 80 with `ANVILML_WORKER_MOCK=1` set as an environment
variable. The worker venv is provisioned from `base.txt`, which deliberately excludes
`torch` (only available via the manual-install `rocm-{linux,windows}.txt` and
`cpu-linux-agent.txt` files). The `conftest.py` forces `ANVILML_WORKER_MOCK=1` via an
autouse fixture for every test.

No Rust crates are touched by this task — it modifies only Python configuration and a
GitHub Actions YAML file. No version bumps are needed.

## Resolved Dependencies

None. This task introduces no new dependencies and references no external crate or package.
The `realcpu` marker is a pure pytest configuration convention — pytest registers markers
declared in `pytest.ini` and silently skips unknown markers at collection time unless
`confcutdir` or `filterwarnings` is configured to error on them.

| Type | Name | Version verified | MCP source | Feature flags confirmed |
|------|------|-----------------|------------|------------------------|
| (n/a) | (n/a) | (n/a) | (n/a) | (n/a) |

## Approach

1. **Update `worker/tests/pytest.ini`**: Append a `markers` line after the existing
   `testpaths` line. The value will be:
   ```
   markers =
       realcpu: real-mode CPU test, requires torch+diffusers+transformers, not run by default CI
   ```
   This is the standard pytest `markers` multi-line format (PEP-compliant: each marker
   gets its own indented line). pytest reads this at collection time and suppresses the
   "unknown option" warning for `@pytest.mark.realcpu` annotations. Tests decorated with
   `@pytest.mark.realcpu` are automatically excluded from any pytest invocation that does
   not explicitly request them (i.e. the default `pytest worker/tests -v` will not collect
   them).

2. **Update `.github/workflows/ci.yml`**: On line 80, change the pytest run command from:
   ```yaml
   run: ${{ matrix.python }} -m pytest worker/tests -v
   ```
   to:
   ```yaml
   run: ${{ matrix.python }} -m pytest worker/tests -v -m "not realcpu"
   ```
   This adds the `-m "not realcpu"` selection expression as defense-in-depth. While the
   absence of `torch` in base.txt already causes collection errors (or test skips) for any
   real-mode test that imports torch, the `-m` expression is an explicit, self-documenting
   guard that makes the exclusion intent clear to any engineer reading the CI config. It
   also protects against a future scenario where `torch` is added to base.txt without
   updating the marker registration.

No other files are modified. No Rust source, no tests, no documentation updates needed
(this task does not add or modify any test functions).

## Public API Surface

None. This task modifies only configuration files (`pytest.ini`, `ci.yml`) — no Python
classes, functions, or Rust pub items are introduced or changed.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | worker/tests/pytest.ini | Add `markers` line registering `realcpu` marker |
| Modify | .github/workflows/ci.yml | Add `-m "not realcpu"` to pytest invocation in worker job |

## Tests

This task modifies configuration files only — no new test functions are written, and no
existing tests are modified. The acceptance criteria below verify the configuration is
correctly applied.

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| (config verification) | pytest.ini marker registration | `pytest.ini` contains the `realcpu` marker definition | File exists at `worker/tests/pytest.ini` | None | grep finds `realcpu` in the file | `grep -q 'realcpu' worker/tests/pytest.ini` exits 0 |
| (config verification) | CI marker exclusion | `ci.yml` worker job includes `-m "not realcpu"` in pytest run | File exists at `.github/workflows/ci.yml` | None | grep finds `-m "not realcpu"` on line 80 | `grep -q 'not realcpu' .github/workflows/ci.yml` exits 0 |

## CI Impact

This task modifies `.github/workflows/ci.yml` — the CI workflow itself. The change adds
`-m "not realcpu"` to the existing pytest invocation in the `worker` job. This has no
impact on current CI behavior (no tests currently use the `realcpu` marker), but establishes
the guard for future test files (P904-Z3, Z4, Z5) that will be annotated with
`@pytest.mark.realcpu`.

No new CI jobs are added. No existing jobs are modified beyond the one line change on
line 80. The `openapi-drift` and `config-drift` jobs are unaffected.

## Platform Considerations

None identified. Both files are platform-neutral: `pytest.ini` is read by the pytest
Python package regardless of host OS, and `.github/workflows/ci.yml` is processed by
GitHub Actions (which runs on Linux or Windows runners via matrix entries, but the YAML
content itself is identical). No `#[cfg(...)]` guards, path separators, or line-ending
differences are involved. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| pytest version does not support multi-line `markers` format | Low | Medium | The multi-line `markers =` format has been part of pytest since v3.0 (2017). The project uses `pytest>=9.1` per base.txt, so this is not a concern. Verify at ACT time by running `worker/.venv/bin/python -m pytest --help | grep -q markers` to confirm the option exists. |
| `-m "not realcpu"` in ci.yml is syntactically incorrect in YAML context | Low | Medium | The value is inside a `${{ matrix.python }}` expansion followed by a plain `-m "not realcpu"` string. The shell receives the full command and passes it to pytest. The double quotes are needed to prevent shell word-splitting of `not realcpu`. Verify at ACT time that the YAML is valid by running `python -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"` — this catches YAML syntax errors before staging. |

## Acceptance Criteria

- [ ] `grep -q 'realcpu' worker/tests/pytest.ini` exits 0
- [ ] `grep -q 'not realcpu' .github/workflows/ci.yml` exits 0
- [ ] `python -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"` exits 0 (valid YAML)
- [ ] `grep -c '^markers' worker/tests/pytest.ini` returns 1 (exactly one markers line)
