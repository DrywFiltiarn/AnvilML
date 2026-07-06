# Plan Report: P12-B1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P12-B1                                      |
| Phase       | 12 — Graph Validation                       |
| Description | anvilml-scheduler: lib.rs re-export pass, 80-line check |
| Depends on  | P12-A6                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-07-06T20:05:00Z                        |
| Attempt     | 1                                           |

## Objective

Confirm that `crates/anvilml-scheduler/src/lib.rs` correctly re-exports all public types and functions produced by Phase 12 (Group A tasks): `ValidatedGraph`, `GraphError`, and `validate_graph`. Verify the file remains within the 80-line hard cap. No implementation logic is added — this is a verification pass only, following the same pattern as every prior crate's closing `lib.rs` task.

## Scope

### In Scope
- Confirm `pub use types::ValidatedGraph;` is present in `lib.rs`.
- Confirm `pub use types::GraphError;` is present in `lib.rs`.
- Confirm `pub use dag::validate_graph;` is present in `lib.rs`.
- Confirm `pub mod dag;` and `pub mod types;` are present (module declarations).
- Verify `wc -l crates/anvilml-scheduler/src/lib.rs` reports ≤ 80.
- Run `cargo test -p anvilml-scheduler` to confirm the full crate test suite exits 0.

### Out of Scope
None. `defers_to (from JSON): []` — this task must implement its full scope. No functionality is deferred.

## Existing Codebase Assessment

The `anvilml-scheduler` crate is at the end of Phase 12. All Group A tasks (P12-A1 through P12-A6) have been completed:

- **`types.rs`** (74 lines): Defines `ValidatedGraph` (construction-gated newtype wrapping `serde_json::Value`) and `GraphError` (7-variant enum with `thiserror::Error` derives). Includes `_test_new()` and `_test_inner()` helper methods for test crate access.
- **`dag.rs`** (471 lines): Implements `validate_graph()` with all six validation checks (structural root, duplicate IDs, unknown node types, dangling edges, slot-type compatibility, cycle detection via Kahn's algorithm) in collect-all-errors mode.
- **`tests/dag_tests.rs`** (1410 lines): Comprehensive test suite covering all six checks individually and in combination.

The `lib.rs` currently contains 7 lines:
```rust
//! Job queue, VRAM ledger, DAG validation, and dispatch loop.

pub mod dag;
pub mod types;
pub use dag::validate_graph;
pub use types::GraphError;
pub use types::ValidatedGraph;
```

All three required re-exports (`ValidatedGraph`, `GraphError`, `validate_graph`) are already present. The module declarations (`pub mod dag`, `pub mod types`) are present. The file is 7 lines, well under the 80-line cap.

The established patterns to follow: `lib.rs` contains only `//!` crate-level doc comment, `pub mod` declarations, and `pub use` re-exports. No implementation code. This pattern is consistent across all other crates in the workspace (anvilml-core, anvilml-ipc, anvilml-worker, etc.).

No gap between the design doc and current source: ANVILML_DESIGN.md §12.1 specifies the module layout with `lib.rs` re-exporting `JobScheduler` and public types. `JobScheduler` does not exist yet (it will be created in Phase 13+), so the current re-exports are correct for Phase 12's scope.

## Resolved Dependencies

None. This task introduces no new dependencies — it only verifies existing re-exports.

## Approach

### Phase Deliverable Audit

Since P12-B1 is the last task in Phase 12's `tasks_phase012.json` (by array order), the end-of-phase deliverable audit (§9a) is mandatory before writing the Approach.

**§9a Procedure — defers_to entries in Phase 12:**
- P12-A3 has `defers_to: ["P12-A4"]`. Verified: P12-A4's description states it implements checks 3–4 (node-type + edge checks), which is exactly what P12-A3 deferred. No finding.
- All other tasks in Phase 12 have `defers_to: []`. No further entries to check.

**§9a.1 Unmarked-stub sweep:**
```bash
grep -rn "NotImplementedError\|unimplemented!\|todo!\|# TODO\|// TODO" crates/anvilml-scheduler/src/
```
Result: `0 findings`. No stubs detected in any source file modified by any Phase 12 task.

**§9a.2 Dual-mode parity-marker sweep:**
The project defines a dual-mode parity marker convention (`REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` pair, ANVILML_DESIGN.md §10.6), but this convention applies exclusively to Python node `execute()` and arch module `load()`/`sample()`/`decode()` functions (see `docs/ENVIRONMENT.md §8, Gate 4`). The `anvilml-scheduler` crate is pure Rust and contains no Python code. No parity markers are applicable.

```bash
grep -rn "REAL_PATH_VERIFIED:\|MOCK_PATH_VERIFIED:" crates/anvilml-scheduler/src/
```
Result: No files found. Convention does not apply to this crate.

**Audit summary:** §9a procedure — 1 defers_to link verified (P12-A3→P12-A4, scope confirmed). §9a.1 — 0 findings. §9a.2 — 0 findings (Python-only convention, Rust crate exempt).

### Step-by-step plan

1. **Read `lib.rs`** (`crates/anvilml-scheduler/src/lib.rs`) and verify it contains:
   - A `//!` crate-level doc comment describing the crate's ownership.
   - `pub mod dag;` and `pub mod types;` module declarations.
   - `pub use dag::validate_graph;` — re-export of the validation function.
   - `pub use types::GraphError;` — re-export of the error enum.
   - `pub use types::ValidatedGraph;` — re-export of the newtype.

2. **Verify line count:** Run `wc -l crates/anvilml-scheduler/src/lib.rs` and confirm the result is ≤ 80. Current count is 7 lines — no risk of exceeding the cap.

3. **Run tests:** Execute `cargo test -p anvilml-scheduler` to confirm the full crate test suite (all 21+ tests in `dag_tests.rs`) exits 0. This validates that no accidental breakage occurred during any prior task's modifications.

4. **If re-exports are missing** (unlikely given current state): Add the missing `pub use` lines following the established pattern from other crates' `lib.rs` files (e.g., `anvilml-ipc/src/lib.rs`, `anvilml-worker/src/lib.rs`).

## Public API Surface

No new public items are introduced. This task only verifies existing re-exports. The crate's public surface for Phase 12 is:

| Item | Path | Kind |
|------|------|------|
| `ValidatedGraph` | `anvilml_scheduler::ValidatedGraph` | `pub struct` (re-exported from `types`) |
| `GraphError` | `anvilml_scheduler::GraphError` | `pub enum` (re-exported from `types`) |
| `validate_graph` | `anvilml_scheduler::validate_graph` | `pub fn` (re-exported from `dag`) |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Read | `crates/anvilml-scheduler/src/lib.rs` | Verify re-exports and line count |

No files are modified. No `Cargo.toml` version bump is needed since no source files change.

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `crates/anvilml-scheduler/tests/dag_tests.rs` | Full suite (21+ tests) | All existing tests pass after this verification task | `cargo test -p anvilml-scheduler` exits 0 |

## CI Impact

No CI changes required. This task only verifies existing code — no new files, no new test modules, no new dependencies, and no CI configuration modifications.

## Platform Considerations

None identified. The `lib.rs` file contains only module declarations and re-exports — no platform-specific code, no `#[cfg(...)]` guards, no path separators, no line-ending handling.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `lib.rs` already contains all required re-exports from prior tasks, so this task has nothing to change — the risk is that the plan over-specifies changes that don't exist. | High | Low | The approach is verification-only. If re-exports are already present (they are), no edits are made. The acceptance criteria (line count ≤ 80, tests pass) are satisfied. |
| A prior task inadvertently added implementation code or extra `pub mod` declarations to `lib.rs`, pushing it toward or past 80 lines. | Low | High | The line count check (`wc -l`) will catch this. If `lib.rs` exceeds 80 lines, the ACT agent must remove non-re-export content (move it to the appropriate module file) before marking complete. |

## Acceptance Criteria

- [ ] `wc -l crates/anvilml-scheduler/src/lib.rs` reports a number ≤ 80
- [ ] `cargo test -p anvilml-scheduler` exits 0 (full crate suite)
