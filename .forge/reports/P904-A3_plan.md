# Plan Report: P904-A3

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P904-A3                                           |
| Phase       | 904 — Test Isolation Hardening                    |
| Description | Verify full workspace test suite green after P904 isolation fixes |
| Depends on  | P904-A1, P904-A2, P904-A2b                        |
| Project     | anvilml                                           |
| Planned at  | 2026-06-11T08:15:00Z                              |
| Attempt     | 1                                                 |

## Objective

Run all six CI gates against the AnvilML workspace to confirm that the P904-A1 (scheduler pool + serial removal), P904-A2 (backend serial removal + multi_thread runtime), and P904-A2b (preflight platform guard) isolation fixes have not introduced regressions. All gates must exit 0. Record per-crate test counts from the workspace test run in the implement report.

## Scope

### In Scope
- Run all six gates sequentially and record exit codes and output:
  1. `cargo test --workspace --features mock-hardware`
  2. `cargo clippy --workspace --features mock-hardware -- -D warnings`
  3. `cargo clippy --bin anvilml -- -D warnings`
  4. `cargo fmt --all -- --check`
  5. `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu`
  6. `cargo check --bin anvilml --target x86_64-pc-windows-gnu`
- Record per-crate test counts from gate 1 output.
- If any gate fails and the failure is directly caused by a P904-A1/A2/A2b change, fix it minimally and re-run the affected gate. Document the fix in the implement report with root-cause explanation.
- If a gate fails for a reason unrelated to P904 changes, document as a blocker and STOP.

### Out of Scope
- No new source code, tests, or configuration files.
- No changes to `Cargo.toml`, `Cargo.lock`, CI workflow files, or documentation.
- No changes to `anvilml-hardware` (serial_test is retained there).
- No Python worker tests (those are covered by a separate CI gate, not part of this task's six gates).
- No OpenAPI drift check (not required for verification-only tasks).

## Approach

1. **Gate 1 — Workspace tests:** Run `cargo test --workspace --features mock-hardware`. Parse the output to extract per-crate test counts (e.g., `running N tests` per crate). Record all output. If any test fails, diagnose whether the failure is a regression from P904-A1/A2/A2b changes (check the failing test's file — if it is one of the files modified by A1/A2/A2b, fix the minimal regression; otherwise STOP and document as blocker).

2. **Gate 2 — Clippy workspace:** Run `cargo clippy --workspace --features mock-hardware -- -D warnings`. All warnings must be eliminated (exit 0). If new warnings appear, determine if they stem from P904 changes and fix minimally.

3. **Gate 3 — Clippy binary:** Run `cargo clippy --bin anvilml -- -D warnings`. This exercises real-hardware (non-mock) code paths. Exit 0 required.

4. **Gate 4 — Format check:** Run `cargo fmt --all -- --check`. Exit 0 required. If non-zero, run `cargo fmt --all` to fix in-place, then re-run the check.

5. **Gate 5 — Windows cross-check (mock):** Run `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu`. Exit 0 required. Exercises `#[cfg(windows)]` cfg-gated mock paths.

6. **Gate 6 — Windows cross-check (real):** Run `cargo check --bin anvilml --target x86_64-pc-windows-gnu`. Exit 0 required. Exercises real-hardware `#[cfg(windows)]` detection paths.

7. **Final verification:** Confirm all six gates exited 0. Summarize per-crate test counts. If all green, mark task COMPLETE.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Read | `.forge/reports/P904-A3_plan.md` | This plan report (only write in this session) |
| Read | `.forge/state/CURRENT_TASK.md` | Update Step/Status to COMPLETE |
| Read | (workspace files) | Read-only inspection of test output; no modifications |

No source, test, config, or CI files are modified by this task. Any fix required due to a P904-caused regression would be documented under `## Deviations from Plan` in the implement report.

## Tests

None. This task does not write any test files. It exercises existing tests via `cargo test --workspace --features mock-hardware` and records the results.

## CI Impact

No CI changes. This task verifies that the existing CI gates (defined in `docs/ARCHITECTURE.md §9`) continue to pass. The six gates match the CI pipeline exactly: format check, clippy (mock + real), workspace tests, and Windows cross-compilation checks.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| A pre-existing test failure unrelated to P904 changes surfaces | Low | High | STOP immediately, document under `## Blockers`, do not attempt to fix |
| A P904-caused regression requires a fix in a file outside the originally listed scope | Low | Medium | Fix minimally, document under `## Deviations from Plan` per FORGE_AGENT_RULES §9.2 |
| `cargo clippy --bin anvilml` fails due to real-hardware code paths not exercised by mock-hardware | Medium | Low | Fix the warning/error; this is expected behavior of the dual-gate clippy approach |
| Windows cross-check fails due to missing mingw toolchain | Low | High | Verify `x86_64-pc-windows-gnu` target and `gcc-mingw-w64` are installed (per ENVIRONMENT.md §7); if missing, STOP and document blocker |
| Test count recording is inaccurate due to output parsing | Low | Low | Use verbatim output; record raw counts from `running N tests` lines per crate |

## Acceptance Criteria

- [ ] `cargo test --workspace --features mock-hardware` exits 0 with zero failures
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0
- [ ] `cargo clippy --bin anvilml -- -D warnings` exits 0
- [ ] `cargo fmt --all -- --check` exits 0
- [ ] `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` exits 0
- [ ] `cargo check --bin anvilml --target x86_64-pc-windows-gnu` exits 0
- [ ] Per-crate test counts recorded in implement report
- [ ] No uncommitted source changes (unless P904-caused regression fix documented)
