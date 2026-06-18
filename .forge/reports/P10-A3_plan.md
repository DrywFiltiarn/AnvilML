# Plan Report: P10-A3

| Field       | Value                                                                        |
|-------------|------------------------------------------------------------------------------|
| Task ID     | P10-A3                                                                       |
| Phase       | 010 — Worker Crash Recovery                                                  |
| Description | anvilml-worker/server: verify Phase 010 implementation and document findings |
| Depends on  | P10-A2                                                                       |
| Project     | anvilml                                                                      |
| Planned at  | 2026-06-18T18:30:00Z                                                         |
| Attempt     | 1                                                                            |

## Objective

Verify that the Phase 010 implementation delivered in P10-A2 is correct and complete by
reading the actual current source files, running the full test suite, and producing a
comprehensive implementation report documenting what exists. No new code is written; the
output is documentation only. At completion: the implementation report accurately describes
the `ManagedWorker` respawn architecture, the `WorkerPool::restart_worker` surface, and
the `POST /v1/workers/{id}/restart` handler as they exist in the current codebase, with
verbatim test output and any discrepancies from the original P10-A2 plan noted under
Deviations.

## Scope

### In Scope

- Read the following files in full before writing anything:
  - `crates/anvilml-worker/src/managed.rs`
  - `crates/anvilml-worker/src/respawn.rs`
  - `crates/anvilml-worker/src/pool.rs`
  - `crates/anvilml-worker/tests/managed_tests.rs`
  - `crates/anvilml-worker/tests/pool_tests.rs`
  - `crates/anvilml-server/src/handlers/workers.rs`
  - `crates/anvilml-server/src/lib.rs`
  - `crates/anvilml-server/tests/workers_tests.rs`
- Run `cargo fmt --all -- --check`
- Run `cargo clippy --workspace --features mock-hardware -- -D warnings`
- Run `cargo test --workspace --features mock-hardware`
- Produce `.forge/reports/P10-A3_implement.md` with verbatim output from all three commands
- Note any discrepancy between what P10-A2 planned and what is actually in the source files
- Update `.forge/state/CURRENT_TASK.md`

### Out of Scope

- Writing or modifying any source file, test, or configuration
- Fixing any defect discovered during verification — if a defect is found, write it under
  `## Blockers` in the report and stop; do not attempt to fix it inline
- Updating `docs/TESTS.md` — this was completed in P10-A2

## Existing Codebase Assessment

Phase 010 implementation (P10-A2) is reported COMPLETE. The following should be present
and verifiable by direct source inspection:

- `ManagedWorker` with seven new fields (`crash_count`, `last_crash`, `cfg`, `device`,
  `transport`, `timeout_rx`, `restart_rx`) and `event_tx: Option<broadcast::Sender<...>>`
- `do_respawn(&mut self, consult_policy: bool)` private async method
- Six-arm `select!` loop in `run()` including heartbeat-timeout and manual-restart arms
- `loop_child` local taken from `self.child` before each `select!` iteration
- `WorkerPool::restart_worker(&self, worker_id: &str) -> Result<(), AnvilError>`
- `POST /v1/workers/{id}/restart` registered in `build_router`
- 12 passing tests in `managed_tests.rs`

The verification task's sole output is a report confirming (or contradicting) this picture
with evidence from actual source reads and test runner output.

## Resolved Dependencies

None. This is a verification-only task with no code changes.

| Type   | Name | Version verified | MCP source | Feature flags confirmed |
|--------|------|-----------------|------------|------------------------|
| (none) | —    | —               | —          | —                      |

## Approach

1. **Read every file listed in In Scope** using the file-read tool. Do not rely on memory
   or prior reports — the P10-A2 synthetic report describes what was intended; this task
   verifies what is actually present. Note the exact struct field list, method signatures,
   and `select!` arm structure.

2. **Run format check**: `cargo fmt --all -- --check`. Record verbatim output.

3. **Run clippy**: `cargo clippy --workspace --features mock-hardware -- -D warnings`.
   Record verbatim output. If any warning is present, write it under `## Blockers`.

4. **Run tests**: `cargo test --workspace --features mock-hardware`. Record verbatim output.
   If any test fails, write the failure under `## Blockers` — do not attempt to fix it.

5. **Compare against P10-A2 plan**: for each item in P10-A2's `## Public API Surface`,
   verify it exists in the source. List any discrepancy under `## Deviations from Plan`.
   Common discrepancies to look for:
   - Fields or methods present in the plan but absent in source (or vice versa)
   - Signature differences (parameter names, types, return types)
   - Structural differences in the `select!` arms
   - Test assertion differences (e.g. `Dead || Respawning` vs `Dead` only)

6. **Write `.forge/reports/P10-A3_implement.md`** with all findings. The `## Summary`
   section must describe the actual architecture found, not what was planned.

## Public API Surface

Not applicable — this task produces no new public items.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `.forge/reports/P10-A3_implement.md` | Verification implementation report |
| MODIFY | `.forge/state/CURRENT_TASK.md` | Update task status |

## Tests

Not applicable — this task runs existing tests but introduces none.

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| (all existing) | (all) | Existing workspace test suite must exit 0 | P10-A2 complete | none | 0 failures | `cargo test --workspace --features mock-hardware` exits 0 |

## CI Impact

No CI changes required.

## Platform Considerations

None identified. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Source files differ from P10-A2 plan in ways that constitute a real defect | Low | High | Write defect under Blockers; do not attempt to fix inline; Phase 011 cannot proceed until resolved |
| Test suite reveals a timing-sensitive failure not seen in prior runs | Low | Medium | Record verbatim output; if failure is deterministic, write as Blocker; if flaky, document with RUST_BACKTRACE=full output |

## Acceptance Criteria

- [ ] `.forge/reports/P10-A3_implement.md` exists and begins with `# Implementation Report: P10-A3`
- [ ] `grep "^## " .forge/reports/P10-A3_implement.md` returns exactly 11 lines
- [ ] `## Test Results` section contains verbatim output showing 0 failures
- [ ] `## Blockers` section is present (contains "None." or a specific described defect)