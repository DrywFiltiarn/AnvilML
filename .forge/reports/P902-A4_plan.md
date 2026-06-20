# Plan Report: P902-A4

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P902-A4                                           |
| Phase       | 902 — ArtifactStore Relocation Retrofit           |
| Description | Repoint ArtifactStore import to anvilml-artifacts in anvilml-server + backend |
| Depends on  | P902-A2                                           |
| Project     | anvilml                                           |
| Planned at  | 2026-06-20T19:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Ensure that `crates/anvilml-server` and `backend` import `ArtifactStore` from the dedicated `anvilml-artifacts` crate (`anvilml_artifacts::ArtifactStore`) rather than from `anvilml-ipc`, while retaining `anvilml-ipc` for `EventBroadcaster` and the IPC message types. This completes the second half of the import repointing split (P902-A3 covers `anvilml-scheduler`, P902-A4 covers `anvilml-server` + `backend`).

## Scope

### In Scope
- Verify `anvilml-artifacts` path dependency exists in `crates/anvilml-server/Cargo.toml` and `backend/Cargo.toml`.
- Verify `anvilml-ipc` path dependency exists in both Cargo.toml files (still needed for `EventBroadcaster`).
- In `crates/anvilml-server/src/state.rs`: confirm `use anvilml_artifacts::ArtifactStore;` (not `anvilml_ipc::ArtifactStore`).
- In `backend/src/main.rs`: confirm `use anvilml_artifacts::ArtifactStore;` and `use anvilml_ipc::{EventBroadcaster, RouterTransport};`.
- In all 11 test files listed below, confirm the import split is correct:
  - `crates/anvilml-server/tests/artifact_store_tests.rs`
  - `crates/anvilml-server/tests/artifacts_tests.rs`
  - `crates/anvilml-server/tests/system_tests.rs`
  - `crates/anvilml-server/tests/jobs_tests.rs`
  - `crates/anvilml-server/tests/nodes_tests.rs`
  - `crates/anvilml-server/tests/state_tests.rs`
  - `crates/anvilml-server/tests/workers_tests.rs`
  - `crates/anvilml-server/tests/handler_tests.rs`
  - `crates/anvilml-server/tests/models_tests.rs`
  - `crates/anvilml-server/tests/health_tests.rs`
- Run `cargo test --workspace --features mock-hardware` and confirm exit 0 with the same test count as before this phase.

### Out of Scope
- Modifying `anvilml-scheduler` (covered by P902-A3).
- Relocating `EventBroadcaster` or any `ws/` module from `anvilml-ipc` (out of scope for the entire Phase 902 retrofit).
- Updating `anvilml-server/src/lib.rs` crate doc comment (covered by P902-A5).
- Adding new functionality or changing `ArtifactStore`'s API.

## Existing Codebase Assessment

The `anvilml-artifacts` crate was created in P902-A1 as a dedicated crate for content-addressed PNG artifact storage, with `ArtifactStore` in `crates/anvilml-artifacts/src/store.rs` and re-exported from `lib.rs`. P902-A2 removed the stale `artifact_store.rs` from `anvilml-ipc` and dropped its dead dependencies (`chrono`, `sha2`, `sqlx`, `base64`).

During P902-A2 implementation, the acting agent merged P902-A3 and P902-A4 into that task because the workspace lint gate requires zero failures — downstream crates could not compile with stale `anvilml_ipc::ArtifactStore` imports. All 15 call sites across `anvilml-scheduler`, `anvilml-server`, and `backend` were updated in that single session, and `anvilml-artifacts` was added as a path dependency to all three crates.

Verification via workspace-wide grep confirms zero `anvilml_ipc::ArtifactStore` references remain in any source or test file. All files now import `anvilml_artifacts::ArtifactStore` separately and retain their `anvilml-ipc` dependency for `EventBroadcaster`, `WorkerEvent`, `WorkerMessage`, and `RouterTransport`.

The `anvilml-ipc` crate's `lib.rs` no longer exports `ArtifactStore` — it only re-exports `EventBroadcaster`, `RouterTransport`, `WorkerEvent`, `WorkerMessage`, decode/encode functions, and transport error types.

## Resolved Dependencies

This task introduces no new external crates. All dependencies are path dependencies within the workspace, already verified in prior phases.

| Type   | Name             | Version verified | MCP source | Feature flags confirmed |
|--------|------------------|-----------------|------------|------------------------|
| crate  | anvilml-artifacts| 0.1.0 (path)    | lockfile   | n/a                    |
| crate  | anvilml-ipc      | 0.1.0 (path)    | lockfile   | n/a                    |

No MCP lookup required — both are local path dependencies already present in the workspace. The task context specifies no external crate version changes.

## Approach

The code is already in the correct state (merged into P902-A2). This task's plan documents the verification steps the ACT agent should execute to confirm correctness and close the task.

1. **Verify Cargo.toml entries in `crates/anvilml-server`.** Confirm `anvilml-artifacts = { path = "../anvilml-artifacts" }` exists in `[dependencies]` and `anvilml-ipc = { path = "../anvilml-ipc" }` also exists. Both must be present.

2. **Verify Cargo.toml entries in `backend`.** Confirm `anvilml-artifacts = { path = "../crates/anvilml-artifacts" }` exists in `[dependencies]` and `anvilml-ipc = { path = "../crates/anvilml-ipc" }` also exists. Both must be present.

3. **Verify source file imports in `crates/anvilml-server/src/state.rs`.** Confirm line 3 reads `use anvilml_artifacts::ArtifactStore;` (not `anvilml_ipc::ArtifactStore`). Confirm no `anvilml_ipc::ArtifactStore` appears anywhere in the file.

4. **Verify source file imports in `backend/src/main.rs`.** Confirm line 19 reads `use anvilml_artifacts::ArtifactStore;` and line 22 reads `use anvilml_ipc::{EventBroadcaster, RouterTransport};`. Confirm no `anvilml_ipc::ArtifactStore` appears anywhere in the file.

5. **Verify test file imports across all 10 anvilml-server test files.** Each must have `use anvilml_artifacts::ArtifactStore;` on its import line. Files that also need `EventBroadcaster` must retain `use anvilml_ipc::EventBroadcaster;` or `use anvilml_ipc::{EventBroadcaster, ...};`. No file may contain `anvilml_ipc::ArtifactStore`.

6. **Run full workspace tests.** Execute `cargo test --workspace --features mock-hardware` and confirm exit 0. Record the test count and compare against the pre-phase baseline.

7. **Run workspace-wide grep for stale references.** Execute `grep -rn "anvilml_ipc::ArtifactStore" crates/anvilml-server/ backend/` and confirm zero matches.

## Public API Surface

No new public items are introduced. This task only changes import paths — the public API surface of `anvilml_artifacts::ArtifactStore` is unchanged from P902-A1.

| Item | Crate | Signature |
|------|-------|-----------|
| `pub struct ArtifactStore` | `anvilml-artifacts` | Already defined in `crates/anvilml-artifacts/src/store.rs` — no change |
| `pub async fn new(...)` | `anvilml-artifacts::ArtifactStore` | Already defined — no change |
| All other `pub` methods | `anvilml-artifacts::ArtifactStore` | Already defined — no change |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Verify | `crates/anvilml-server/Cargo.toml` | Confirm `anvilml-artifacts` and `anvilml-ipc` deps present |
| Verify | `backend/Cargo.toml` | Confirm `anvilml-artifacts` and `anvilml-ipc` deps present |
| Verify | `crates/anvilml-server/src/state.rs` | Confirm `anvilml_artifacts::ArtifactStore` import |
| Verify | `backend/src/main.rs` | Confirm `anvilml_artifacts::ArtifactStore` + `anvilml_ipc` imports |
| Verify | `crates/anvilml-server/tests/artifact_store_tests.rs` | Confirm import split |
| Verify | `crates/anvilml-server/tests/artifacts_tests.rs` | Confirm import split |
| Verify | `crates/anvilml-server/tests/system_tests.rs` | Confirm import split |
| Verify | `crates/anvilml-server/tests/jobs_tests.rs` | Confirm import split |
| Verify | `crates/anvilml-server/tests/nodes_tests.rs` | Confirm import split |
| Verify | `crates/anvilml-server/tests/state_tests.rs` | Confirm import split |
| Verify | `crates/anvilml-server/tests/workers_tests.rs` | Confirm import split |
| Verify | `crates/anvilml-server/tests/handler_tests.rs` | Confirm import split |
| Verify | `crates/anvilml-server/tests/models_tests.rs` | Confirm import split |
| Verify | `crates/anvilml-server/tests/health_tests.rs` | Confirm import split |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| Workspace | All tests | Full workspace test suite passes with mock-hardware | `cargo test --workspace --features mock-hardware` exits 0 |
| Workspace | Stale reference grep | No `anvilml_ipc::ArtifactStore` references remain in server or backend | `grep -rn "anvilml_ipc::ArtifactStore" crates/anvilml-server/ backend/` returns no matches |

## CI Impact

No CI changes required. The test suite (`cargo test --workspace --features mock-hardware`) already runs on every CI job (`rust-linux`, `rust-windows`). No new test files, gate, or configuration are introduced. The existing CI jobs will pick up the verified state automatically.

## Platform Considerations

None identified. This task only changes import paths — no platform-specific code, no `#[cfg(unix)]` / `#[cfg(windows)]` guards, no path-separator handling, and no line-ending differences. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| The code was already merged into P902-A2, so this task has no implementation work — the ACT agent may incorrectly assume nothing needs to be done and mark the task complete without running the verification commands. | High | Medium | The acceptance criteria explicitly require running `cargo test --workspace --features mock-hardware` and the stale-reference grep. The agent must execute these and record output before marking COMPLETE. |
| A future developer or phase task inadvertently reintroduces `anvilml_ipc::ArtifactStore` in a new file within `anvilml-server` or `backend`. | Low | High | The acceptance criteria include a workspace-wide grep check. The ACT agent should run `grep -rn "anvilml_ipc::ArtifactStore" crates/anvilml-server/ backend/` and confirm zero matches. |
| `anvilml-ipc` dependency is accidentally removed from one of the Cargo.toml files when cleaning up, breaking `EventBroadcaster` compilation. | Low | High | The acceptance criteria verify both `anvilml-artifacts` AND `anvilml-ipc` are present in both Cargo.toml files. |

## Acceptance Criteria

- [ ] `grep "anvilml-artifacts" crates/anvilml-server/Cargo.toml` returns at least one match
- [ ] `grep "anvilml-ipc" crates/anvilml-server/Cargo.toml` returns at least one match
- [ ] `grep "anvilml-artifacts" backend/Cargo.toml` returns at least one match
- [ ] `grep "anvilml-ipc" backend/Cargo.toml` returns at least one match
- [ ] `grep "anvilml_ipc::ArtifactStore" crates/anvilml-server/src/state.rs` returns no matches
- [ ] `grep "anvilml_artifacts::ArtifactStore" crates/anvilml-server/src/state.rs` returns exactly one match
- [ ] `grep "anvilml_ipc::ArtifactStore" backend/src/main.rs` returns no matches
- [ ] `grep "anvilml_artifacts::ArtifactStore" backend/src/main.rs` returns exactly one match
- [ ] `grep -rn "anvilml_ipc::ArtifactStore" crates/anvilml-server/tests/ backend/tests/` returns no matches
- [ ] `grep -rn "anvilml_artifacts::ArtifactStore" crates/anvilml-server/tests/` returns at least 10 matches (one per test file)
- [ ] `cargo test --workspace --features mock-hardware` exits 0
