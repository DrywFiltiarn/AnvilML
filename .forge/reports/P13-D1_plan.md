# Plan Report: P13-D1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P13-D1                                       |
| Phase       | 13 — Job Queue                               |
| Description | anvilml-scheduler: lib.rs re-export pass, 80-line check |
| Depends on  | P13-A2, P13-A3                               |
| Project     | anvilml                                      |
| Planned at  | 2026-07-07T11:30:00Z                         |
| Attempt     | 1                                            |

## Objective

Confirm that `crates/anvilml-scheduler/src/lib.rs` already contains the correct
`pub use` re-exports for all Phase 12 and Phase 13 types per ANVILML_DESIGN.md §12.1's
module layout, and verify the file stays within the 80-line hard cap. No new
implementation logic is added — this is a verification and, if needed, correction pass
on the crate's public surface declaration.

## Scope

### In Scope
- Inspect the current content of `crates/anvilml-scheduler/src/lib.rs`.
- Confirm `pub use` re-exports for the following five items are present:
  - `ValidatedGraph` (from `types`, Phase 12)
  - `GraphError` (from `types`, Phase 12)
  - `validate_graph` (from `dag`, Phase 12)
  - `JobQueue` (from `queue`, Phase 13)
  - `VramLedger` (from `ledger`, Phase 13)
- Confirm `pub mod` declarations for `dag`, `ledger`, `queue`, and `types` are present.
- Verify the file is ≤ 80 lines.
- Run the full `anvilml-scheduler` test suite and confirm exit 0.

### Out of Scope
None. This task's `defers_to` is `[]` — no scope is deferred. The crate's `scheduler.rs`
and `event_loop.rs` modules (referenced in ANVILML_DESIGN.md §12.1 but not yet created —
they belong to later phases) are not part of this task's scope.

## Existing Codebase Assessment

The current `lib.rs` (10 lines) already contains all five required re-exports and four
`pub mod` declarations:

```rust
//! Job queue, VRAM ledger, DAG validation, and dispatch loop.

pub mod dag;
pub mod ledger;
pub mod queue;
pub mod types;
pub use dag::validate_graph;
pub use ledger::VramLedger;
pub use queue::JobQueue;
pub use types::GraphError;
pub use types::ValidatedGraph;
```

This matches ANVILML_DESIGN.md §12.1's module layout exactly: `types.rs` (Phase 12's
`ValidatedGraph`/`GraphError`), `dag.rs` (Phase 12's `validate_graph`), `queue.rs`
(Phase 13's `JobQueue`), and `ledger.rs` (Phase 13's `VramLedger`). The file is 10
lines — well under the 80-line cap. The crate's three test files (`queue_tests.rs`,
`ledger_tests.rs`, `dag_tests.rs`) exercise all four modules' public APIs.

No gap between the design doc and current source was found. The `lib.rs` is already
correct for this phase's deliverables.

## Resolved Dependencies

None. This task introduces no new dependencies and references no external crate types,
method names, or feature flags. All types re-exported (`ValidatedGraph`, `GraphError`,
`validate_graph`, `JobQueue`, `VramLedger`) are local to the `anvilml-scheduler` crate
and were verified via source inspection, not MCP.

## Approach

1. **Read current `lib.rs`** (already done during codebase inspection). Confirm the
   five `pub use` re-exports and four `pub mod` declarations match the expected set.

2. **Verify completeness.** Compare against ANVILML_DESIGN.md §12.1's module layout:
   - `pub mod dag;` → provides `validate_graph` ✓
   - `pub mod ledger;` → provides `VramLedger` ✓
   - `pub mod queue;` → provides `JobQueue` ✓
   - `pub mod types;` → provides `ValidatedGraph` and `GraphError` ✓
   - All five `pub use` lines present ✓
   - No extraneous items ✓

3. **Verify 80-line cap.** `wc -l crates/anvilml-scheduler/src/lib.rs` must report ≤ 80.
   Current count is 10 lines.

4. **Run test suite.** `cargo test -p anvilml-scheduler` must exit 0. This runs the
   three integration test files (`queue_tests.rs`, `ledger_tests.rs`, `dag_tests.rs`)
   plus any inline tests.

5. **No changes needed** if all checks pass. The `lib.rs` is already correct.

## Public API Surface

No new public items are introduced. The existing `pub use` declarations remain unchanged:

| Item | Source module | Type |
|------|--------------|------|
| `JobQueue` | `queue::JobQueue` | struct |
| `VramLedger` | `ledger::VramLedger` | struct |
| `ValidatedGraph` | `types::ValidatedGraph` | struct |
| `GraphError` | `types::GraphError` | enum |
| `validate_graph` | `dag::validate_graph` | fn |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Read | `crates/anvilml-scheduler/src/lib.rs` | Verify re-exports and line count |

No files are created or modified — the current `lib.rs` is already correct.

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `crates/anvilml-scheduler/tests/queue_tests.rs` | All existing queue tests | JobQueue FIFO order, cancel, get, list, len, is_empty | `cargo test -p anvilml-scheduler --test queue_tests` exits 0 |
| `crates/anvilml-scheduler/tests/ledger_tests.rs` | All existing ledger tests | VramLedger reserve, release, free_mib, over-release safety | `cargo test -p anvilml-scheduler --test ledger_tests` exits 0 |
| `crates/anvilml-scheduler/tests/dag_tests.rs` | All existing dag tests | Graph validation checks 1–6 | `cargo test -p anvilml-scheduler --test dag_tests` exits 0 |
| (full suite) | `cargo test -p anvilml-scheduler` | Full crate test suite exits 0 | `cargo test -p anvilml-scheduler` exits 0 |

## CI Impact

No CI changes required. No new files, test modules, or build configurations are added.
The existing CI gate (`cargo test --workspace --features mock-hardware`) already covers
this crate.

## Platform Considerations

None identified. The `lib.rs` file is platform-neutral — it contains only module
declarations and re-exports. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| A previous phase's task modified `lib.rs` and introduced a duplicate or conflicting re-export that clippy would flag | Low | Medium | `cargo test -p anvilml-scheduler` will surface any compilation or lint errors; run it as the acceptance step. |
| The `wc -l` count is misleading because of trailing whitespace or encoding issues | Low | Low | `wc -l` counts newline characters, which is the standard metric used by the project's CI and this task's acceptance criterion. |

## Acceptance Criteria

- [ ] `wc -l crates/anvilml-scheduler/src/lib.rs` exits 0 and the printed number is ≤ 80
- [ ] `cargo test -p anvilml-scheduler` exits 0 (full crate suite)
