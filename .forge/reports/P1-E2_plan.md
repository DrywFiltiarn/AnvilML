# Plan Report: P1-E2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P1-E2                                       |
| Phase       | 001 — Repository Scaffold                   |
| Description | CI: ci.yml worker-test matrix + drift job placeholders |
| Depends on  | P1-E1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-26T14:30:00Z                        |
| Attempt     | 1                                           |

## Objective

Append three placeholder job blocks to the existing `.github/workflows/ci.yml` — `worker-test` (a 4-entry matrix covering ubuntu/windows × mock/real), `openapi-drift`, and `config-drift` — each running a simple `echo` that exits 0. This completes the CI workflow file's full 8-entry design (from `ANVILML_DESIGN.md §18.3` / `ENVIRONMENT.md §6 GitHub CI job matrix`) so that all planned jobs are defined and green, even though the underlying subsystems (`worker/`, OpenAPI generation, config drift checking) are not yet implemented.

## Scope

### In Scope
- Modify `.github/workflows/ci.yml` to add three new job blocks:
  - `worker-test`: strategy.matrix.include with 4 entries (`{os: ubuntu-latest, mode: mock}`, `{os: ubuntu-latest, mode: real}`, `{os: windows-latest, mode: mock}`, `{os: windows-latest, mode: real}`), one job body, placeholder step `echo "worker tests: no worker/ source yet (mode=${{ matrix.mode }})"` exiting 0.
  - `openapi-drift`: single job (no matrix), placeholder step `echo "no openapi/config yet"` exiting 0.
  - `config-drift`: single job (no matrix), placeholder step `echo "no openapi/config yet"` exiting 0.
- Preserve the existing `rust-test` job block unchanged.
- All four jobs (`rust-test`, `worker-test`, `openapi-drift`, `config-drift`) must be present in the final file.

### Out of Scope
None. This task has `defers_to: []` and implements its full scope. The worker-test, openapi-drift, and config-drift jobs will become real pytest/OpenAPI/config-drift invocations in later phases (Phase 7 for worker, later for openapi-drift and config-drift), but the placeholder jobs themselves are fully implemented here.

## Existing Codebase Assessment

No prior source code is modified by this task — only `.github/workflows/ci.yml` is changed. That file was created by P1-E1 and contains a single `rust-test` job with a 2-entry OS matrix (ubuntu-latest, windows-latest). The file uses the `dtolnay/rust-toolchain@master` action and runs `cargo fmt --all -- --check` (Linux-only), `cargo clippy`, and `cargo test`.

The established patterns to follow:
- Job ordering: `rust-test` first, then `worker-test`, then `openapi-drift`, then `config-drift` (matching the design doc's table order in §18.3).
- Each job block is a top-level mapping under `jobs:` with `runs-on:`, `steps:`, and consistent 2-space indentation.
- The `if:` gating pattern for Linux-only steps (used by rust-test's format check) should be replicated where needed.
- Matrix entries use `strategy.matrix.include` (not `strategy.matrix.os`) for the worker-test job since it has two matrix axes (os + mode).

No gap between the design doc and current source affects this task — the existing ci.yml is a clean starting point, and the design doc's CI table (§18.3) precisely specifies the shape of the three new jobs.

## Resolved Dependencies

None. This task modifies only a YAML workflow file and introduces no external dependencies, packages, or crates.

## Approach

1. **Read existing `.github/workflows/ci.yml`** to confirm its current state (the `rust-test` job block with 5 steps: checkout, install Rust, format check (Linux-only), clippy, test).

2. **Append the `worker-test` job** — a single job body driven by a matrix with 4 explicit entries via `strategy.matrix.include`:
   ```yaml
   worker-test:
     runs-on: ${{ matrix.os }}
     strategy:
       matrix:
         include:
           - os: ubuntu-latest
             mode: mock
           - os: ubuntu-latest
             mode: real
           - os: windows-latest
             mode: mock
           - os: windows-latest
             mode: real
     steps:
       - name: Checkout repository
         uses: actions/checkout@v4
       - name: Run worker tests
         run: echo "worker tests: no worker/ source yet (mode=${{ matrix.mode }})"
   ```
   Rationale: The design doc (§18.3) specifies four separate jobs in the final CI, but at this phase the `worker/` directory does not exist (Phase 7 scope). Using a single matrix-driven job body avoids duplicating the job across 4 nearly-identical blocks while still producing 4 green checkmarks in the CI UI.

3. **Append the `openapi-drift` job** — a single job (no matrix):
   ```yaml
   openapi-drift:
     runs-on: ubuntu-latest
     steps:
       - name: Checkout repository
         uses: actions/checkout@v4
       - name: Check openapi drift
         run: echo "no openapi/config yet"
   ```
   Rationale: Placeholder only. When `anvilml-openapi` is fully implemented, this job will run `cargo run -p anvilml-openapi && git diff --exit-code api/openapi.json`.

4. **Append the `config-drift` job** — a single job (no matrix):
   ```yaml
   config-drift:
     runs-on: ubuntu-latest
     steps:
       - name: Checkout repository
         uses: actions/checkout@v4
       - name: Check config drift
         run: echo "no openapi/config yet"
   ```
   Rationale: Placeholder only. When `ServerConfig` and the `config_reference` test exist, this job will run `cargo test -p anvilml --features mock-hardware -- config_reference`.

5. **Verify the final file** — confirm that `grep -c 'runs-on' .github/workflows/ci.yml` returns 4 (rust-test uses matrix, worker-test uses matrix, openapi-drift and config-drift each have one `runs-on`).

## Public API Surface

None. This task modifies a CI workflow YAML file and introduces no public Rust/Python API items.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `.github/workflows/ci.yml` | Append `worker-test`, `openapi-drift`, and `config-drift` placeholder job blocks |

## Tests

This task modifies a CI YAML file — no source code tests are needed. The acceptance criteria are mechanical grep checks against the file structure, which are verifiable shell commands (listed in `## Acceptance Criteria`).

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `.github/workflows/ci.yml` | ci_structure_check | Exactly 4 `runs-on` lines present (rust-test + worker-test matrix counts as 2, openapi-drift + config-drift = 2) | `grep -c 'runs-on' .github/workflows/ci.yml` exits 0 with output `4` |
| `.github/workflows/ci.yml` | worker_test_matrix_check | `worker-test` job exists with `strategy.matrix.include` and 4 entries | `grep -A20 'worker-test:' .github/workflows/ci.yml | grep -c 'mode:'` exits 0 with output `4` |
| `.github/workflows/ci.yml` | drift_jobs_check | Both `openapi-drift` and `config-drift` job names present | `grep -c 'openapi-drift:\|config-drift:' .github/workflows/ci.yml` exits 0 with output `2` |
| `.github/workflows/ci.yml` | rust_test_unchanged | `rust-test` job block still present with all original steps | `grep -c 'rust-test:' .github/workflows/ci.yml` exits 0 with output `1` |

## CI Impact

This task IS the CI change. It adds three new jobs to the GitHub Actions CI workflow:
- `worker-test` — 4 green checkmarks on push (matrix entries), currently placeholder echo.
- `openapi-drift` — 1 green checkmark on push, currently placeholder echo.
- `config-drift` — 1 green checkmark on push, currently placeholder echo.

The existing `rust-test` job is preserved unchanged. The total job count goes from 2 matrix entries (rust-test on ubuntu + windows) to 8 matrix entries (rust-test: 2 + worker-test: 4 + openapi-drift: 1 + config-drift: 1 = 8 distinct CI checkmarks).

## Platform Considerations

None identified. The `worker-test` matrix covers both `ubuntu-latest` and `windows-latest` runners, matching the design doc's four-job split (`worker-linux-mock`, `worker-linux-real`, `worker-windows-mock`, `worker-windows-real`). The placeholder echo commands are platform-neutral.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| YAML indentation drift — appending new job blocks with incorrect indentation breaks the YAML parse, causing all CI jobs to fail silently | Low | High | Copy the exact 2-space indentation style from the existing `rust-test` block; verify with `python -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"` before committing |
| `runs-on` count mismatch — the acceptance criterion expects exactly 4 `runs-on` lines, but if a job block is malformed or duplicated, the count will be wrong | Low | Medium | Run the grep count check immediately after writing; the heredoc approach ensures all four blocks are written atomically |
| The existing `rust-test` job is accidentally modified or its steps reordered | Low | High | The plan only appends new job blocks after the existing file content — no edits to the rust-test block are made. The heredoc reads the existing file and appends |

## Acceptance Criteria

- [ ] `grep -c 'runs-on' .github/workflows/ci.yml` outputs `4` (exit 0)
- [ ] `grep 'worker-test:' .github/workflows/ci.yml` matches (exit 0)
- [ ] `grep 'openapi-drift:' .github/workflows/ci.yml` matches (exit 0)
- [ ] `grep 'config-drift:' .github/workflows/ci.yml` matches (exit 0)
- [ ] `grep -A20 'worker-test:' .github/workflows/ci.yml | grep -c 'mode:'` outputs `4` (exit 0)
- [ ] `grep 'rust-test:' .github/workflows/ci.yml` matches (exit 0, rust-test preserved)
- [ ] `python3 -c "import yaml; yaml.safe_load(open('.github/workflows/ci.yml'))"` exits 0 (valid YAML)
