# Plan Report: P7-B1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P7-B1                                             |
| Phase       | 007 — WebSocket Event Stream                      |
| Description | anvilml: add real-hardware lint steps to rust-linux and rust-windows CI jobs |
| Depends on  | P7-A5                                             |
| Project     | anvilml                                           |
| Planned at  | 2026-06-04T19:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add a `Real-hardware lint` step to both the `rust-linux` and `rust-windows` CI jobs in `.github/workflows/ci.yml`, placed immediately after each job's existing `Real-hardware compile check` step. This closes the lint gap where real-hardware code paths (`#[cfg(unix)]` on Linux, `#[cfg(windows)]` on Windows) were never scanned by clippy.

## Scope

### In Scope
- Edit `.github/workflows/ci.yml` only
- Add one new step to `rust-linux` job: `Real-hardware lint` running `cargo clippy --bin anvilml -- -D warnings`
- Add one new step to `rust-windows` job: `Real-hardware lint` running `cargo clippy --bin anvilml -- -D warnings`
- No `--features` flag on either step (real-hardware paths, not mock)
- All existing jobs and steps preserved unchanged

### Out of Scope
- No changes to any source code, test, or config file
- No changes to any other CI workflow file
- No changes to the `python-worker`, `openapi-diff`, or any other job
- No dependency version changes
- No modification to Cargo.toml files

## Approach

1. Open `.github/workflows/ci.yml`.
2. In the `rust-linux` job, locate the existing step:
   ```yaml
   - name: Real-hardware compile check
     run: cargo check --bin anvilml
   ```
3. Insert immediately before it a new step block:
   ```yaml

       - name: Real-hardware lint
         run: cargo clippy --bin anvilml -- -D warnings
   ```
4. In the `rust-windows` job, locate the existing step:
   ```yaml
   - name: Real-hardware compile check
     run: cargo check --bin anvilml
   ```
5. Insert immediately before it a new step block (same as Linux):
   ```yaml

       - name: Real-hardware lint
         run: cargo clippy --bin anvilml -- -D warnings
   ```
6. Verify the file still has valid YAML structure and that both jobs are unchanged except for the added steps.
7. Acceptance check: `grep -c 'Real-hardware lint' .github/workflows/ci.yml` must print `2`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Edit | `.github/workflows/ci.yml` | Add `Real-hardware lint` step to `rust-linux` and `rust-windows` jobs |

## Tests

None. This task modifies only a CI workflow file; no source code or test files are touched. The acceptance criterion is a grep count on the YAML file, not a test runner.

## CI Impact

Two additional steps are added to two existing CI jobs (`rust-linux` and `rust-windows`). Each step runs `cargo clippy --bin anvilml -- -D warnings`, which compiles only the `anvilml` binary crate without the `mock-hardware` feature, exercising real-platform code paths. This adds a small amount of lint time to each affected job but does not introduce new jobs or change any existing job's behavior. No CI infrastructure changes are required.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Pre-existing clippy warnings in real-hardware code paths cause the new CI step to fail | The task context confirms P6-B2 already added the compile check; if clippy fails, fix the minimal warnings — this is expected and part of the task's purpose. Record under deviations. |
| Accidentally modifying another job or step | Carefully scope edits to only the `rust-linux` and `rust-windows` jobs; verify with grep that no other jobs contain "Real-hardware lint" after edit. |
| YAML indentation error breaks the workflow file | Use the exact indentation pattern from surrounding steps (2-space YAML indent, 8-space for nested run lines matching existing style). Validate by re-reading the file after edit. |

## Acceptance Criteria

- [ ] `grep -c 'Real-hardware lint' .github/workflows/ci.yml` returns `2`
- [ ] All pre-existing jobs and steps in `.github/workflows/ci.yml` are preserved unchanged (no deletions, no renames, no reordering)
- [ ] The new step appears immediately after the `Real-hardware compile check` step in both `rust-linux` and `rust-windows` jobs
