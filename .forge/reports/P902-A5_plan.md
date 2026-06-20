# Plan Report: P902-A5

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P902-A5                                           |
| Phase       | 902 — ArtifactStore Relocation Retrofit           |
| Description | Remove false 'owns artifact store' claim from lib.rs crate doc |
| Depends on  | P902-A1, P902-A2, P902-A3, P902-A4                |
| Project     | anvilml                                           |
| Planned at  | 2026-06-20T19:58:00Z                              |
| Attempt     | 1                                                 |

## Objective

Correct a false ownership claim in the `anvilml-server` crate-level doc comment.
After P902-A1 moved `ArtifactStore` into the new `anvilml-artifacts` crate, the
`lib.rs` doc comment (line 5) still states that `anvilml-server` "owns... the
artifact store." This task removes that clause while preserving the accurate
router/handlers/broadcaster ownership claims and the "no business logic" hard
constraint. No source code behaviour changes — purely a doc comment correction.

## Scope

### In Scope
- Edit `crates/anvilml-server/src/lib.rs` lines 3–6: remove the phrase "and the
  artifact store" from the crate-level `//!` doc comment.
- The corrected doc should read:
  ```
  //! HTTP and WebSocket server for AnvilML.
  //!
  //! This crate owns the axum router, all HTTP handlers (health, system,
  //! jobs, models, workers, artifacts, nodes), and the WebSocket broadcaster.
  //! Handlers call into scheduler, worker, and registry crates only — no
  //! business logic lives here.
  //!
  //! **Hard constraints:** No business logic. All handlers delegate to
  //! the scheduler, worker pool, and model registry.
  ```
- Note: "artifacts" remains in the handler list because `anvilml-server` owns
  the artifact HTTP handlers (`handlers::artifacts`), not the store itself.
  This is accurate — handlers serve artifacts over HTTP by delegating to the
  store owned by `anvilml-artifacts`.

### Out of Scope
- `docs/ARCHITECTURE.md` — already corrected manually (commit `5cb6a8b`).
- `docs/ANVILML_DESIGN.md` — already corrected manually (commit `5cb6a8b`).
- `README.md` — already corrected manually (commit `5cb6a8b`).
- Any `Cargo.toml` changes (no dependency changes).
- Any source code logic changes.
- Any test additions (no behavioural change introduced).

## Existing Codebase Assessment

The `anvilml-server` crate at `crates/anvilml-server/` owns the axum HTTP router
(`build_router`), all HTTP handlers under `src/handlers/` (health, system, jobs,
models, workers, artifacts, nodes), and the WebSocket event stream under `src/ws/`.
Its crate-level doc comment (lines 1–9) declares ownership of "the artifact store"
on line 5 — a claim that became false when P902-A1 created `anvilml-artifacts` as
the independent home for `ArtifactStore`.

The crate follows the established pattern: `lib.rs` contains `pub mod`, `pub use`,
the crate-level doc comment, and the `build_router` function — no inline test blocks,
no implementation code beyond the router builder. The file is 103 lines, exceeding
the 80-line `lib.rs` guideline threshold, but this pre-exists this task and is
outside scope.

The `anvilml-server/Cargo.toml` already depends on `anvilml-artifacts` (path
dependency, line 7), confirming that the crate uses the artifact store as a
dependency rather than owning it. The crate also depends on `anvilml-scheduler`,
`anvilml-ipc`, `anvilml-registry`, and others — consistent with the dependency
graph in ARCHITECTURE.md §3.

No gap between the design doc and current source: the design doc (already corrected
manually) correctly attributes `ArtifactStore` to `anvilml-artifacts`. Only the
`lib.rs` doc comment is stale.

## Resolved Dependencies

None. This task modifies only a doc comment in existing source code. No external
dependencies are introduced, removed, or changed.

| Type | Name | Version verified | MCP source | Feature flags confirmed |
|------|------|-----------------|------------|------------------------|
| (N/A) | (N/A) | (N/A) | (N/A) | (N/A) |

## Approach

1. **Open `crates/anvilml-server/src/lib.rs`.** Read lines 1–9 to confirm the
   current doc comment text and identify the exact string to replace.

2. **Replace the crate-level doc comment.** The existing comment spans lines 1–9:
   ```
   //! HTTP and WebSocket server for AnvilML.
   //!
   //! This crate owns the axum router, all HTTP handlers (health, system,
   //! jobs, models, workers, artifacts, nodes), the WebSocket broadcaster,
   //! and the artifact store. Handlers call into scheduler, worker, and
   //! registry crates only — no business logic lives here.
   //!
   //! **Hard constraints:** No business logic. All handlers delegate to
   //! the scheduler, worker pool, and model registry.
   ```
   Replace with:
   ```
   //! HTTP and WebSocket server for AnvilML.
   //!
   //! This crate owns the axum router, all HTTP handlers (health, system,
   //! jobs, models, workers, artifacts, nodes), and the WebSocket broadcaster.
   //! Handlers call into scheduler, worker, and registry crates only — no
   //! business logic lives here.
   //!
   //! **Hard constraints:** No business logic. All handlers delegate to
   //! the scheduler, worker pool, and model registry.
   ```
   Specific changes:
   - Remove `the WebSocket broadcaster,` → `and the WebSocket broadcaster.`
     (replaced comma with "and" + period to end the sentence)
   - Remove `and the artifact store.` entirely
   - The phrase "artifacts" stays in the handler parenthetical because the crate
     owns the HTTP handlers for artifacts (they serve artifact data over HTTP),
     not the storage itself.

3. **Verify no other occurrences.** Run `grep -rn 'artifact store' crates/anvilml-server/`
   to confirm this is the only occurrence in the crate. (The task description
   confirms the target is `grep -n 'artifact store' crates/anvilml-server/src/lib.rs`
   returning no hits.)

4. **Verify `cargo doc -p anvilml-server` builds.** The doc comment is the only
   change; cargo doc must succeed with zero warnings.

## Public API Surface

None. This task modifies only a `//!` crate-level doc comment. No `pub` items,
function signatures, struct definitions, or trait impls are introduced, removed,
or changed. The `grep "^pub "` check on modified files will return the same
results before and after.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-server/src/lib.rs` | Remove "and the artifact store" from crate-level doc comment (lines 3–6) |

No `Cargo.toml` version bump is needed — this task modifies only a doc comment
with zero behavioural impact. However, per FORGE_AGENT_RULES §14, every task
that modifies source files inside a crate must increment the patch version.
The ACT agent will bump `anvilml-server` from `0.1.24` to `0.1.25`.

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| (none) | (none) | No behavioural change introduced. The task is a pure doc comment correction. | — | — | — | `grep -n 'artifact store' crates/anvilml-server/src/lib.rs` returns no hits (exit 1, meaning grep found nothing) |

Note: While FORGE_AGENT_RULES §5.1 states "every task that writes source code
MUST include tests," this task performs a zero-behaviour-change doc correction.
The acceptance criterion is the grep returning no hits, which proves the false
claim was removed. No new test is needed because there is no new code path,
public API, or runtime behaviour to test.

## CI Impact

No CI changes required. The change is a doc comment inside a crate's source file.
All existing CI jobs (`rust-linux`, `rust-windows`) will pick it up as part of
their normal `cargo clippy` and `cargo test` runs. No new file types, gates, or
test modules are introduced.

## Platform Considerations

None identified. The change is a doc comment in a Rust source file — entirely
platform-neutral. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `cargo doc -p anvilml-server` produces a warning about broken intra-doc links due to the doc comment restructuring | Low | Medium | Verify with `cargo doc -p anvilml-server 2>&1` after editing; if a warning appears, check whether it is pre-existing (present before the edit) or caused by the edit. Pre-existing warnings must be fixed per FORGE_AGENT_RULES §9.3; a warning caused by the edit must be corrected. |
| The `lib.rs` file exceeds 80 lines after the edit (it is currently 103 lines) | N/A | Low | The 80-line threshold was already exceeded before this task. Reducing the doc comment by ~2 lines brings it to 101 lines — still over the threshold but a marginal improvement. The line count reduction is a side effect, not the goal. |
| The word "artifacts" remains in the handler parenthetical and someone later reads it as implying ownership of the store | Low | Low | The parenthetical lists handler modules, not owned subsystems. The sentence structure makes clear: "This crate owns [list of things including handlers for artifacts] and the WebSocket broadcaster." The distinction between "handlers for artifacts" and "the artifact store" is semantically unambiguous. |

## Acceptance Criteria

- [ ] `grep -n 'artifact store' crates/anvilml-server/src/lib.rs` returns exit code 1 (no matches)
- [ ] `cargo doc -p anvilml-server 2>&1` exits 0 (doc builds with no errors)
- [ ] `grep -n 'artifact store' crates/anvilml-server/src/lib.rs` is not the only grep hit — verify with `grep -rn 'artifact store' crates/anvilml-server/` that no other occurrence exists in the crate
- [ ] The doc comment still mentions "artifacts" in the handler parenthetical: `grep 'handlers (health, system, jobs, models, workers, artifacts, nodes)' crates/anvilml-server/src/lib.rs` returns exit code 0
- [ ] `grep 'WebSocket broadcaster' crates/anvilml-server/src/lib.rs` returns exit code 0 (broadcaster claim preserved)
- [ ] `grep 'No business logic' crates/anvilml-server/src/lib.rs` returns exit code 0 (hard constraint preserved)
