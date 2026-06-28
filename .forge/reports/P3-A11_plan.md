# Plan Report: P3-A11

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P3-A11                                      |
| Phase       | 3 — Core Domain Types: Data Model           |
| Description | anvilml-core: lib.rs final re-export pass, 80-line check |
| Depends on  | P3-A10                                      |
| Project     | anvilml                                     |
| Planned at  | 2026-06-28T19:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Perform a final re-export pass on `crates/anvilml-core/src/lib.rs`: confirm it contains only a `//!` crate-level doc comment, `pub mod` declarations, `mod` (private) declarations, and `pub use` statements — with no implementation code. Reorder all declarations alphabetically by module path. Confirm the file is ≤80 lines per ANVILML_DESIGN.md §4.1 and FORGE_AGENT_RULES.md §12.3/§13. The task is tagged `refactor` and makes zero observable behaviour changes.

## Scope

### In Scope
- Read `crates/anvilml-core/src/lib.rs` and confirm its current contents.
- Reorder module declarations and `pub use` statements alphabetically by module path.
- Confirm the file contains no implementation code (no function bodies, no `impl` blocks, no top-level expressions).
- Confirm the line count is ≤80.
- Run `cargo test -p anvilml-core` and confirm zero regressions.

### Out of Scope
None. This task's `defers_to` field is `[]` (absent of any deferrals). No scope is deferred.

## Existing Codebase Assessment

`crates/anvilml-core/src/lib.rs` currently contains 17 lines. It has a two-line `//!` crate doc comment, five module declarations (`mod config;`, `pub mod config_load;`, `mod error;`, `mod node_registry;`, `pub mod types;`), four `pub use` statements, and one glob re-export. The modules are already in alphabetical order: `config`, `config_load`, `error`, `node_registry`, `types`. The individual `pub use` items within each group are also alphabetically sorted by their full path. No implementation code exists in this file.

The `types/mod.rs` submodule (15 lines) declares seven `pub mod` entries (`artifact`, `events`, `hardware`, `job`, `model`, `node`, `worker`) and re-exports each with a glob `pub use`. All Phase 2 and Phase 3 modules are present and accounted for.

No gap exists between the design doc and current source for this task. The file is already in a clean state — the task is a verification pass that may result in zero changes.

## Resolved Dependencies

None. This task introduces no new dependencies and references no external crate types, method names, or feature flags. It only touches `lib.rs` within `anvilml-core`, which is an internal crate with no new external API to verify.

## Approach

1. **Read and inspect** `crates/anvilml-core/src/lib.rs` (already done). Verify:
   - The file contains only `//!` doc comments, `mod` declarations, `pub mod` declarations, and `pub use` statements.
   - No `impl` blocks, function bodies, or top-level expressions exist.
   - No `#[cfg(...)]` guards are needed beyond what already exists.

2. **Verify alphabetical ordering** of all module declarations and `pub use` statements by full module path:
   - `config` (private): `mod config;`
   - `config_load` (public): `pub mod config_load;`
   - `error` (private): `mod error;`
   - `node_registry` (private): `mod node_registry;`
   - `types` (public): `pub mod types;`
   - `pub use` items within each group, sorted by the path segment after `pub use`:
     - `config::ServerConfig`
     - `config_load::CliOverrides`
     - `config_load::load`
     - `error::AnvilError`
     - `node_registry::NodeTypeRegistry`
     - `types::*`

   Current file already matches this ordering. No reordering is needed.

3. **Confirm line count**: `wc -l crates/anvilml-core/src/lib.rs` must report ≤80. Current count is 17 lines — well under the cap.

4. **Run tests**: `cargo test -p anvilml-core` must exit 0 with no regressions. This confirms the re-export structure (or lack of changes) does not break any downstream crate that depends on these public exports.

5. **Refactor verification** (FORGE_AGENT_RULES.md §4.6): Run `grep -n "^pub " crates/anvilml-core/src/lib.rs` and confirm no public signature changed. Since this task makes zero changes (or only reorders existing lines), this is a no-op confirmation.

## Public API Surface

No new or modified public items. The existing pub surface remains identical:

| Item | Module Path |
|------|-------------|
| `ServerConfig` | `anvilml_core::config::ServerConfig` |
| `CliOverrides` | `anvilml_core::config_load::CliOverrides` |
| `load` | `anvilml_core::config_load::load` |
| `AnvilError` | `anvilml_core::error::AnvilError` |
| `NodeTypeRegistry` | `anvilml_core::node_registry::NodeTypeRegistry` |
| `types::*` (glob) | `anvilml_core::types::*` — includes all domain types |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| READ | crates/anvilml-core/src/lib.rs | Inspect and verify re-export ordering; may reorder if needed |
| READ | crates/anvilml-core/src/types/mod.rs | Verify submodule declarations are present |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| (existing) | `cargo test -p anvilml-core` | Full anvilml-core test suite passes with no regressions | None | N/A | All tests pass, exit 0 | `cargo test -p anvilml-core` exits 0 |

No new tests are added — this is a pure reordering/refactoring task. The existing test suite (`job_tests`, `model_tests`, `artifact_tests`, `hardware_tests`, `worker_tests`, `node_tests`, `events_tests`, `node_registry_tests`) serves as regression verification.

## CI Impact

No CI changes required. The task modifies only `lib.rs` re-exports in `anvilml-core` and makes zero observable behaviour changes. All existing CI jobs (`rust-linux`, `rust-windows`) pick up the crate's test suite automatically.

## Platform Considerations

None identified. The file contains only Rust module declarations and re-exports — no platform-specific code, no `#[cfg(unix)]` / `#[cfg(windows)]` guards, no path separators. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Reordering changes the public API surface order, potentially breaking downstream crates that rely on import order for documentation rendering | Low | Low | The Rust compiler does not care about import order; `pub use` statements are semantically equivalent regardless of ordering. No downstream crate will break. |
| The file is already in correct order and no changes are needed — the task becomes a no-op verification pass | High | None | This is acceptable. The task's acceptance criteria (line count ≤80, tests pass) still apply and will be verified. |
| A module was accidentally added to lib.rs with implementation code that should live in a separate file | Low | Medium | Step 1 explicitly checks for `impl` blocks and function bodies. If found, the plan flags them as defects to be investigated rather than split arbitrarily. |

## Acceptance Criteria

- [ ] `wc -l crates/anvilml-core/src/lib.rs` reports a number ≤80
- [ ] `cargo test -p anvilml-core` exits 0
- [ ] `grep -n "^pub " crates/anvilml-core/src/lib.rs` confirms no public signature changed (refactor verification per FORGE_AGENT_RULES.md §4.6)
- [ ] `grep -rn "impl \|fn \|fn\b" crates/anvilml-core/src/lib.rs` returns no matches (no implementation code in lib.rs)

## Phase Deliverable Audit

This is the phase-closing task (last entry in `tasks_phase003.json`). Per FORGE_AGENT_RULES.md §9a, §9a.1, and §9a.2, the following mechanical audits were run:

### §9a — defers_to coverage verification

Tasks in Phase 3 with non-empty `defers_to`:
- P3-A8: `defers_to: ["P3-A9"]`

Verification:
- P3-A9's description: "anvilml-core: WsEvent worker/system/provisioning variants" — context states it "extends the WsEvent enum in crates/anvilml-core/src/types/events.rs with the variants deferred by P3-A8". This genuinely covers the deferred scope.
- `grep -rn "defers_to:" crates/anvilml-core/src/` returned 0 findings — P3-A8/P3-A9 are data type definitions, not stub code. No `// defers_to:` comment marker is needed on data types.

### §9a.1 — Unmarked-stub sweep

```bash
grep -rn "NotImplementedError\|unimplemented!\|todo!\|# TODO\|// TODO" crates/anvilml-core/
# Result: 0 findings (exit code 1)
```

No unmarked stubs found in the phase's source files.

### §9a.2 — Dual-mode parity-marker sweep

The parity marker convention (ANVILML_DESIGN.md §10.6) applies to node `execute()` and arch-module `load()`/`sample()`/`decode()`/`compute_latent_shape()` functions in `worker/nodes/`. Phase 3 operates exclusively on `anvilml-core`, which contains no Python worker node files.

```bash
grep -L "REAL_PATH_VERIFIED:" crates/anvilml-core/src/lib.rs
# Result: crates/anvilml-core/src/lib.rs (expected — lib.rs is a re-export file, not a function definition)
grep -L "MOCK_PATH_VERIFIED:" crates/anvilml-core/src/lib.rs
# Result: crates/anvilml-core/src/lib.rs (expected — same reason)
```

These results are not findings: `lib.rs` does not define any function in the parity-marker convention's scope. The convention applies to `worker/nodes/` Python files, which Phase 3 does not touch.

**Dual-mode parity-marker sweep: 0 findings.**
