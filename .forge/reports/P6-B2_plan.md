# Plan Report: P6-B2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P6-B2                                       |
| Phase       | 006 — Model Registry                        |
| Description | anvilml: add real-hardware compile check steps to rust-linux and rust-windows CI jobs |
| Depends on  | P6-B1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-04T09:35:00Z                        |
| Attempt     | 1                                           |

## Objective

Lock in the real-hardware compile guarantee by adding a `cargo check --bin anvilml` step to both the `rust-linux` and `rust-windows` CI jobs, ensuring that `#[cfg(unix)]` and `#[cfg(windows)]` code paths are exercised on every CI run.

## Scope

### In Scope
- Add one new step named "Real-hardware compile check" (running `cargo check --bin anvilml`) to the `rust-linux` job, immediately after its existing "Run tests" step
- Add one new step named "Real-hardware compile check" (running `cargo check --bin anvilml`) to the `rust-windows` job, immediately after its existing "Run tests" step
- Preserve all existing jobs, steps, names, commands, and ordering unchanged

### Out of Scope
- Modifying any other CI workflow file
- Adding the step to any job other than `rust-linux` and `rust-windows`
- Adding or removing any features flags on the new steps
- Changing existing step names, commands, or positions
- Modifying any source code, tests, or configuration files

## Approach

1. Open `.github/workflows/ci.yml`.
2. In the `rust-linux` job, insert a new step block immediately after the "Run tests" step (line 30–31):
   ```yaml
       - name: Real-hardware compile check
         run: cargo check --bin anvilml
   ```
3. In the `rust-windows` job, insert an identical step block immediately after the "Run tests" step (line 46–47).
4. Verify the file still has valid YAML structure and that both steps are present by running:
   ```bash
   grep -c 'Real-hardware compile check' .github/workflows/ci.yml
   ```
   Expected output: `2`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Edit | `.github/workflows/ci.yml` | Add "Real-hardware compile check" step to `rust-linux` and `rust-windows` jobs |

## Tests

None. This task modifies only a CI workflow file; no test files are written or changed.

## CI Impact

The CI pipeline adds one step per job (two total) that runs `cargo check --bin anvilml` without any feature flags. On `rust-linux` (`ubuntu-latest`) this exercises the `#[cfg(unix)]` real-hardware detection paths natively. On `rust-windows` (`windows-latest`, native MSVC toolchain) it exercises the `#[cfg(windows)]` paths — matching what a real user would run. The step is fast (incremental check, no test compilation) and adds minimal CI time. All existing jobs (`python-worker`, `openapi-diff`) are unaffected.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Real-hardware code paths have compile errors that were not fixed in P6-B1 (e.g. due to environment differences) | P6-B2 depends on P6-B1; if the check fails, CI will catch it early and block regressions. The agent should fix only errors introduced by this task's scope. |
| `cargo check --bin anvilml` on `ubuntu-latest` may pull in native dependencies (e.g. Vulkan loader, libdrm) not present in the GitHub Actions environment | `cargo check` does not link or run code — it only compiles and type-checks. Native library headers are not required for a check-only pass. If header dependencies appear, they indicate a build-system issue that should be fixed in P6-B1. |
| Accidentally modifying existing steps while inserting the new step | The edit is strictly additive: insert exactly two YAML step blocks at known line positions without touching any other lines. Verification with `grep -c` confirms exactly two occurrences. |

## Acceptance Criteria

- [ ] `grep -c 'Real-hardware compile check' .github/workflows/ci.yml` returns `2`
- [ ] All existing jobs (`rust-linux`, `rust-windows`, `python-worker`, `openapi-diff`) and their steps are preserved unchanged
- [ ] The new step appears immediately after "Run tests" in both `rust-linux` and `rust-windows`
- [ ] No features flag is present on either new step (command is exactly `cargo check --bin anvilml`)
