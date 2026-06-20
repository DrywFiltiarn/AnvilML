# Plan Report: P902-A3

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P902-A3                                           |
| Phase       | 902 — ArtifactStore Relocation Retrofit           |
| Description | Repoint ArtifactStore import to anvilml-artifacts |
| Depends on  | P902-A2                                           |
| Project     | anvilml                                           |
| Planned at  | 2026-06-20T19:00:00Z                              |
| Attempt     | 1                                                 |

## Objective

Split every `use anvilml_ipc::{ArtifactStore, ...}` import in `anvilml-scheduler` source and test files into two separate imports — `use anvilml_artifacts::ArtifactStore;` and the remaining `anvilml_ipc` import — after `ArtifactStore` was relocated into the new `anvilml-artifacts` crate by P902-A1 and removed from `anvilml-ipc` by P902-A2. This ensures `anvilml-scheduler` depends on `anvilml-artifacts` directly and no longer references `ArtifactStore` through the IPC crate.

## Scope

### In Scope
- `crates/anvilml-scheduler/Cargo.toml` — verify `anvilml-artifacts` path dependency is present (it is).
- `crates/anvilml-scheduler/src/event_loop.rs` — verify `ArtifactStore` is imported from `anvilml_artifacts`, not `anvilml_ipc`.
- `crates/anvilml-scheduler/src/scheduler.rs` — verify `ArtifactStore` is imported from `anvilml_artifacts`, not `anvilml_ipc`.
- `crates/anvilml-scheduler/tests/scheduler_tests.rs` — verify import path.
- `crates/anvilml-scheduler/tests/dispatch_tests.rs` — verify import path.
- `crates/anvilml-scheduler/tests/image_ready_tests.rs` — verify import path.
- `crates/anvilml-scheduler/tests/event_loop_tests.rs` — verify import path.
- `cargo test -p anvilml-scheduler --features mock-hardware` exits 0 with the same test count as before.

### Out of Scope
- `anvilml-server`/`backend` crate import repointing (covered by P902-A4).
- Any changes to `ArtifactStore`'s internal implementation, method signatures, or public API.
- Changes to `anvilml-ipc` (P902-A2 already removed `ArtifactStore` from that crate).

## Existing Codebase Assessment

Codebase inspection of all 6 files listed in the task's Files Affected table reveals that **the import split is already in place**. Every source and test file in `anvilml-scheduler` already imports `ArtifactStore` from `anvilml_artifacts` (not `anvilml_ipc`), and retains the `anvilml_ipc` dependency for `EventBroadcaster`, `WorkerEvent`, and `WorkerMessage` where needed.

Specifically:
- `src/event_loop.rs` line 21: `use anvilml_artifacts::ArtifactStore;` — correct.
- `src/event_loop.rs` line 23: `use anvilml_ipc::{EventBroadcaster, WorkerEvent};` — correct, retains IPC dependency.
- `src/scheduler.rs` line 16: `use anvilml_artifacts::ArtifactStore;` — correct.
- `src/scheduler.rs` line 20: `use anvilml_ipc::{EventBroadcaster, WorkerMessage};` — correct, retains IPC dependency.
- All 4 test files (`scheduler_tests.rs`, `dispatch_tests.rs`, `image_ready_tests.rs`, `event_loop_tests.rs`) each have `use anvilml_artifacts::ArtifactStore;` on their import lines.
- No file in the scheduler crate contains `anvilml_ipc::ArtifactStore`.

The `anvilml-scheduler/Cargo.toml` already declares `anvilml-artifacts = { path = "../anvilml-artifacts" }` as a dependency (line 7), and `anvilml-ipc = { path = "../anvilml-ipc" }` remains as a dependency (line 10) for the still-needed `EventBroadcaster` and message types.

The `anvilml-artifacts` crate's `lib.rs` correctly re-exports `pub use store::ArtifactStore;`. The `anvilml-ipc` crate's `lib.rs` does **not** re-export `ArtifactStore` (it was removed by P902-A2).

No gap or discrepancy exists between the design doc and current source — the code is already in the target state.

## Resolved Dependencies

| Type   | Name              | Version verified | MCP source | Feature flags confirmed |
|--------|-------------------|-----------------|------------|------------------------|
| crate  | anvilml-artifacts | N/A (path dep)  | N/A        | none                   |
| crate  | anvilml-ipc       | N/A (path dep)  | N/A        | none                   |

All dependencies are workspace path dependencies — no external crate version resolution needed. The `anvilml-artifacts` crate was created in P902-A1 and already exists with the correct structure.

## Approach

**Assessment:** Inspection confirms the task's required changes are already present in the codebase. All 6 files already have the correct import split, and `anvilml-scheduler/Cargo.toml` already declares the `anvilml-artifacts` dependency.

The following steps verify the current state is correct and produce the acceptance evidence:

1. **Verify Cargo.toml** — confirm `anvilml-artifacts` path dependency is present (already confirmed at line 7). No change needed.
2. **Verify source imports** — confirm `event_loop.rs` and `scheduler.rs` import `ArtifactStore` from `anvilml_artifacts` (already confirmed at lines 21 and 16 respectively). No change needed.
3. **Verify test imports** — confirm all 4 test files import `ArtifactStore` from `anvilml_artifacts` (already confirmed at line 15/16 in each file). No change needed.
4. **Verify no stale references** — grep confirms zero `anvilml_ipc::ArtifactStore` references in the scheduler crate. No cleanup needed.
5. **Run tests** — execute `cargo test -p anvilml-scheduler --features mock-hardware` to confirm the full test suite passes with the current import structure. This is the acceptance gate.

No source code changes, no new dependencies, no API modifications. The task is a verification and test-run of already-correct code.

## Public API Surface

None. No new public items are introduced. No existing public signatures are changed. `ArtifactStore`'s struct shape, method signatures (`new`, `save`, `get`, `list`), and behaviour are unchanged — only its crate of residence was different before P902-A1/A2, and that change is already in effect.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Verify | crates/anvilml-scheduler/Cargo.toml | `anvilml-artifacts` path dep already present |
| Verify | crates/anvilml-scheduler/src/event_loop.rs | `ArtifactStore` import already from `anvilml_artifacts` |
| Verify | crates/anvilml-scheduler/src/scheduler.rs | `ArtifactStore` import already from `anvilml_artifacts` |
| Verify | crates/anvilml-scheduler/tests/scheduler_tests.rs | `ArtifactStore` import already from `anvilml_artifacts` |
| Verify | crates/anvilml-scheduler/tests/dispatch_tests.rs | `ArtifactStore` import already from `anvilml_artifacts` |
| Verify | crates/anvilml-scheduler/tests/image_ready_tests.rs | `ArtifactStore` import already from `anvilml_artifacts` |
| Verify | crates/anvilml-scheduler/tests/event_loop_tests.rs | `ArtifactStore` import already from `anvilml_artifacts` |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| (existing) | All tests in `crates/anvilml-scheduler/tests/` | The full scheduler test suite compiles and passes with the current import structure | `cargo test -p anvilml-scheduler --features mock-hardware` exits 0 |

## CI Impact

No CI changes required. This task touches only import paths within an existing crate — no new files, no new test modules, no new CI gates. The existing `rust-linux` and `rust-windows` CI jobs will pick up the test run via `cargo test --workspace --features mock-hardware`.

## Platform Considerations

None identified. The Windows cross-check in ENVIRONMENT.md §7 is sufficient. Import path changes are platform-neutral — Rust module resolution uses forward slashes on all platforms and is unaffected by OS-specific path separators.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The task context describes changes that have already been applied — the ACT agent may attempt redundant edits that introduce no-op changes or conflicts. | High | Low | The plan explicitly states all changes are already in place. The ACT agent should run the acceptance test command and verify zero diffs before staging. |
| A future phase task inadvertently reintroduces `anvilml_ipc::ArtifactStore` in a new file within the scheduler crate. | Low | Medium | The grep check (`anvilml_ipc.*ArtifactStore`) should be part of the acceptance criteria to catch any stale references. |
| The `anvilml-artifacts` dependency was added but the crate's `lib.rs` does not re-export `ArtifactStore`, causing a compile error. | Low | High | Verified: `anvilml-artifacts/src/lib.rs` contains `pub use store::ArtifactStore;` — the re-export is correct. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-scheduler --features mock-hardware` exits 0
- [ ] `grep -rn "anvilml_ipc::ArtifactStore" crates/anvilml-scheduler/` returns no matches (zero stale references)
- [ ] `grep -rn "anvilml_artifacts::ArtifactStore" crates/anvilml-scheduler/` returns exactly 6 matches (one per file listed in Files Affected)
- [ ] `grep "anvilml-artifacts" crates/anvilml-scheduler/Cargo.toml` returns a match (dependency declared)
- [ ] `grep "anvilml-ipc" crates/anvilml-scheduler/Cargo.toml` returns a match (IPC dependency retained for EventBroadcaster/WorkerEvent)
