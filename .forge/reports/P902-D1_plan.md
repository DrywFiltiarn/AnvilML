# Plan Report: P902-D1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P902-D1                                           |
| Phase       | 902 — Stabilisation Retrofit                      |
| Description | Full workspace stabilisation gate                  |
| Depends on  | P902-B3                                           |
| Project     | anvilml                                            |
| Planned at  | 2026-06-08T18:45:00Z                              |
| Attempt     | 1                                                  |

## Objective

Verify the AnvilML workspace passes all four stabilisation gates with zero warnings and zero test failures, without modifying any source files. This gate task confirms that all prerequisite tasks in Phase 902 (Groups A and B) have left the codebase in a clean state.

## Scope

### In Scope
- Run `cargo clippy --workspace --features mock-hardware -- -D warnings` and verify exit code 0 with zero warnings
- Run `env -i HOME=$HOME PATH=$PATH ANVILML_WORKER_MOCK=1 ANVILML_VENV_PATH=./worker/.venv cargo test --workspace --features mock-hardware` and verify exit code 0 with zero failures
- Run `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` and verify exit code 0
- Run `python -m pytest worker/tests/ -v` and verify exit code 0 with zero failures
- Record all four verbatim outputs in the implementation report

### Out of Scope
- Any source code changes
- Any test file modifications
- Any configuration file changes
- Any dependency version updates
- Any git operations (commit, push, branch)

## Approach

1. **Gate 1 — Clippy lint:** Run `cargo clippy --workspace --features mock-hardware -- -D warnings` and capture full stdout/stderr. Verify exit code is 0 and no warning lines appear.

2. **Gate 2 — Rust test suite:** Run `env -i HOME=$HOME PATH=$PATH ANVILML_WORKER_MOCK=1 ANVILML_VENV_PATH=./worker/.venv cargo test --workspace --features mock-hardware` and capture full stdout/stderr. Verify exit code is 0 and no test failures appear. The `env -i` clears the ambient environment to ensure tests are not dependent on external variables beyond those explicitly set.

3. **Gate 3 — Windows cross-check:** Run `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` and capture full stdout/stderr. Verify exit code is 0. This exercises the `#[cfg(windows)]` code paths via cross-compilation from Linux.

4. **Gate 4 — Python worker tests:** Run `python -m pytest worker/tests/ -v` and capture full stdout/stderr. Verify exit code is 0 and no test failures appear.

5. **Report assembly:** Write the implementation report with all four verbatim outputs as the body content. The task is COMPLETE only when all four commands exit 0.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| No changes | — | This task modifies no source, test, config, or CI files |

## Tests

None. This task does not write or modify any test files. It runs the existing test suites as verification gates.

## CI Impact

No CI changes required. The four commands executed in this gate are already documented as CI gates in `docs/ARCHITECTURE.md §9` and `docs/ENVIRONMENT.md §6`. No workflow file modifications are needed.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| A prerequisite task (P902-A1 through P902-B3) left the codebase in a failing state | Medium | High — gate fails, ACT must stop and report blocker | If any gate exits non-zero, document the verbatim output under `## Blockers` and STOP immediately. Do not attempt fixes. |
| Missing cross-compilation target (`x86_64-pc-windows-gnu`) | Low | High — Gate 3 fails | Verify `rustup target add x86_64-pc-windows-gnu` is available in the toolchain. If missing, document as blocker. |
| Python test environment not configured (missing pytest or worker venv) | Low | Medium — Gate 4 fails | Ensure `python` resolves to a working interpreter with `pytest` installed. If not, document as blocker. |
| Pre-existing clippy warnings from earlier phases | Medium | High — Gate 1 fails | Since this is a gate task (no source changes), any failure is a blocker: document and STOP. |

## Acceptance Criteria

- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 with zero warnings
- [ ] `env -i HOME=$HOME PATH=$PATH ANVILML_WORKER_MOCK=1 ANVILML_VENV_PATH=./worker/.venv cargo test --workspace --features mock-hardware` exits 0 with zero test failures
- [ ] `cargo check --workspace --features mock-hardware --target x86_64-pc-windows-gnu` exits 0
- [ ] `python -m pytest worker/tests/ -v` exits 0 with zero test failures
