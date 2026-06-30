# Plan Report: P900-A10

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P900-A10                                    |
| Phase       | 900 — Spec-Drift & Logging Retrofit         |
| Description | anvilml-core: fix ProvisioningState's variant names to match ANVILML_DESIGN.md |
| Depends on  | P900-A9                                     |
| Project     | anvilml                                     |
| Planned at  | 2026-06-30T18:56:00Z                        |
| Attempt     | 1                                           |

## Objective

Rename two variants of `ProvisioningState` in `crates/anvilml-core/src/types/worker.rs` so they match the design document's specification: `InProgress` → `Provisioning`, `Complete` → `Ready`. This is a pure rename — the enum's role (tracking a worker's provisioning lifecycle, gated by P28-A1's startup check) is unchanged. Update the existing roundtrip test in `worker_tests.rs` for the renamed variants and their new snake_case JSON wire values (`"provisioning"`, `"ready"`).

## Scope

### In Scope
- Rename `ProvisioningState::InProgress` → `Provisioning` in `crates/anvilml-core/src/types/worker.rs`.
- Rename `ProvisioningState::Complete` → `Ready` in `crates/anvilml-core/src/types/worker.rs`.
- Keep `ProvisioningState::NotStarted` and `ProvisioningState::Failed` as-is.
- Update `crates/anvilml-core/tests/worker_tests.rs`'s `test_provisioning_state_serde_snake_case` test: replace `ProvisioningState::InProgress` with `ProvisioningState::Provisioning` and `ProvisioningState::Complete` with `ProvisioningState::Ready`, and update the expected JSON strings from `"in_progress"`/`"complete"` to `"provisioning"`/`"ready"`.
- Update `crates/anvilml-core/tests/worker_tests.rs`'s `test_env_report_serde_roundtrip` test: the `provisioning` field currently uses `ProvisioningState::NotStarted` (unchanged) and asserts `"not_started"` (unchanged), so no change needed there.

### Out of Scope
defers_to (from JSON): [] — this task may not defer any scope.

None. All functionality described in the task context is implemented in full.

## Existing Codebase Assessment

The `ProvisioningState` enum exists at `crates/anvilml-core/src/types/worker.rs` with four variants: `NotStarted`, `InProgress`, `Complete`, `Failed`. It derives `Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema` and uses `#[serde(rename_all = "snake_case")]`. The `EnvReport` struct (already corrected by P900-A9) includes a `provisioning: ProvisioningState` field.

The test file `crates/anvilml-core/tests/worker_tests.rs` contains a dedicated roundtrip test `test_provisioning_state_serde_snake_case` that iterates over all four variants, asserts each serialises to its snake_case JSON string, and deserialises back. The test also includes an `EnvReport` roundtrip test that uses `ProvisioningState::NotStarted`.

Established patterns: derive lists follow a consistent order (`Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema` for enums); `#[serde(rename_all = "snake_case")]` is used on all state enums; tests construct variants directly and assert JSON roundtrips. No logging, tracing, or external API calls are involved.

No gap between the design doc and current source beyond the variant names themselves. The design doc (§5.7) specifies `Ready, Provisioning, Failed, NotStarted` — the current code uses `NotStarted, InProgress, Complete, Failed`. The field order in the design doc differs from the current code, but since `serde(rename_all = "snake_case")` controls wire format (not variant order), and the enum derives `PartialEq` for value equality, only the variant names matter.

## Resolved Dependencies

None. This task introduces no new dependencies and references no external crate types or APIs. The only types involved (`ProvisioningState`, `EnvReport`) are defined within `anvilml-core` itself.

## Approach

1. **Open `crates/anvilml-core/src/types/worker.rs`.** Locate the `ProvisioningState` enum (lines 89–98). Rename variant `InProgress` to `Provisioning` and variant `Complete` to `Ready`. The doc comments for each variant remain semantically correct (the `InProgress` comment "The provisioning process is currently running" applies equally to `Provisioning`; the `Complete` comment "All required dependencies are installed and verified" applies equally to `Ready`). Do not modify doc comments, derive attributes, or `#[serde(rename_all = "snake_case")]`.

2. **Update `crates/anvilml-core/tests/worker_tests.rs`.** In `test_provisioning_state_serde_snake_case` (lines 81–86), replace the variant/JSON pairs:
   - `ProvisioningState::InProgress, "in_progress"` → `ProvisioningState::Provisioning, "provisioning"`
   - `ProvisioningState::Complete, "complete"` → `ProvisioningState::Ready, "ready"`
   Keep `NotStarted`/`"not_started"` and `Failed`/`"failed"` unchanged.

3. **Verify `test_env_report_serde_roundtrip`** (lines 113–155). The test constructs `EnvReport` with `provisioning: ProvisioningState::NotStarted` and asserts `parsed["provisioning"] == "not_started"`. Since `NotStarted` is unchanged, no modification needed.

4. **Run acceptance commands:** `cargo test -p anvilml-core --test worker_tests` and `cargo doc -p anvilml-core --no-deps`. Both must exit 0.

defers_to (from JSON): []

### Phase Deliverable Audit

P900-A10 is the last task (10th of 10) in phase 900. Per FORGE_AGENT_RULES.md §9a, §9a.1, and §9a.2, the following audits are mandatory before writing the Approach:

**§9a procedure — defers_to chain audit:**
All 10 tasks in `tasks_phase900.json` have `defers_to: []` (empty). No task defers scope to another. The procedure finds zero findings — no owner-to-stub links to verify.

**§9a.1 Unmarked-stub sweep:**
```bash
grep -rn "NotImplementedError\|unimplemented!\|todo!\|# TODO\|// TODO" crates/anvilml-core/src/types/worker.rs crates/anvilml-core/tests/worker_tests.rs backend/src/main.rs backend/src/cli.rs backend/src/shutdown.rs backend/tests/logging_tests.rs backend/tests/db_startup_tests.rs crates/anvilml-server/src/handlers/health.rs crates/anvilml-server/tests/health_tests.rs crates/anvilml-core/src/types/job.rs crates/anvilml-core/src/types/model.rs
```
Expected result: 0 findings. The phase's source files do not contain any unmarked stubs. (The grep will be run at ACT time to confirm; the plan records the command and expected outcome.)

**§9a.2 Dual-mode parity-marker sweep:**
The project defines a `REAL_PATH_VERIFIED`/`MOCK_PATH_VERIFIED` marker convention (`ANVILML_DESIGN.md §10.6`), but it applies to Python node functions (`execute()`, `load()`, `sample()`, `decode()`, `compute_latent_shape()`). This task modifies only Rust source files in `anvilml-core` — no Python node files are touched. The parity-marker sweep finds no applicable files.

Result: `"Dual-mode parity-marker sweep: 0 findings"`

## Public API Surface

| Item | Crate/Module | Change |
|------|-------------|--------|
| `ProvisioningState::InProgress` → `ProvisioningState::Provisioning` | `anvilml-core::types::ProvisioningState` | Variant renamed |
| `ProvisioningState::Complete` → `ProvisioningState::Ready` | `anvilml-core::types::ProvisioningState` | Variant renamed |

No new `pub` items are introduced. No existing `pub` signature changes (only variant names within an existing `pub enum`). The `#[serde(rename_all = "snake_case")]` attribute causes the JSON wire values to change automatically: `"in_progress"` → `"provisioning"`, `"complete"` → `"ready"`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/src/types/worker.rs` | Rename `ProvisioningState::InProgress` → `Provisioning`, `Complete` → `Ready` |
| Modify | `crates/anvilml-core/tests/worker_tests.rs` | Update `test_provisioning_state_serde_snake_case` for renamed variants and new JSON strings |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `crates/anvilml-core/tests/worker_tests.rs` | `test_provisioning_state_serde_snake_case` | All four `ProvisioningState` variants (`NotStarted`, `Provisioning`, `Failed`, `Ready`) serialise to correct snake_case JSON and roundtrip. Updated for renamed variants. | `cargo test -p anvilml-core --test worker_tests` exits 0 |
| `crates/anvilml-core/tests/worker_tests.rs` | `test_env_report_serde_roundtrip` | `EnvReport` with `provisioning: ProvisioningState::NotStarted` roundtrips correctly; `"not_started"` JSON string verified. Unchanged by this task. | `cargo test -p anvilml-core --test worker_tests` exits 0 |
| `crates/anvilml-core/tests/worker_tests.rs` | All worker tests | No regression in any existing test in the worker test suite. | `cargo test -p anvilml-core --test worker_tests` exits 0 |
| `crates/anvilml-core/` | `cargo doc` | New variant names compile and docs generate without error. | `cargo doc -p anvilml-core --no-deps` exits 0 |

## CI Impact

No CI changes required. The test suite is already configured to pick up `crates/anvilml-core/tests/worker_tests.rs` via `cargo test --workspace --features mock-hardware`. The variant rename is a pure source change with no new file types, gates, or test modules.

## Platform Considerations

None identified. The enum variants and their serde serialization are platform-neutral. The `#[serde(rename_all = "snake_case")]` attribute produces identical output on all platforms. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `cargo test --workspace` in other crates references `ProvisioningState` variants by name (e.g., `ProvisioningState::InProgress`) and fails to compile after the rename. | High | High | The grep at ACT time will find all references. Any occurrence in other crates must be updated as part of this task. The task description says this is a pure rename, so all references should be updated. |
| The `EnvReport` roundtrip test in `worker_tests.rs` asserts the JSON string `"not_started"` for `ProvisioningState::NotStarted`. Since `NotStarted` is unchanged, this is unaffected — but if a future task changes the `EnvReport` to use a different variant, the test should be updated then. | Low | Low | This is informational only. The test currently passes and will continue to pass. |
| The design doc lists variants in order `Ready, Provisioning, Failed, NotStarted` while the current code lists them as `NotStarted, InProgress, Complete, Failed`. Renaming changes the order to `NotStarted, Provisioning, Ready, Failed`. | Low | Low | The enum's variant order has no semantic meaning — `serde` uses `rename_all = "snake_case"` for wire format, and `PartialEq` compares values, not order. The design doc's order is documentation, not a contract. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-core --test worker_tests` exits 0
- [ ] `cargo doc -p anvilml-core --no-deps` exits 0
