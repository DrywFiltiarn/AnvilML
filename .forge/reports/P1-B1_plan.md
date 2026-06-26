# Plan Report: P1-B1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P1-B1                                       |
| Phase       | 001 — Repository Scaffold                   |
| Description | anvilml-core: empty crate, compiles, in workspace |
| Depends on  | P1-A1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-26T11:45:00Z                        |
| Attempt     | 1                                           |

## Objective

Create the root crate of AnvilML's dependency graph — `anvilml-core` — as an empty,
doc-commented stub that compiles cleanly. This crate owns all pure domain types, config
schemas, and error enums for the project. It must have zero external dependencies and
zero I/O/async code. Adding it to the workspace members list enables every subsequent
crate task (P1-B2 through P1-B6) to declare a path dependency on it.

## Scope

### In Scope
- Create `crates/anvilml-core/Cargo.toml` with `[package] name = "anvilml-core"`,
  workspace-inherited `version` and `edition`, `rust-version.workspace = true`,
  and no dependencies.
- Create `crates/anvilml-core/src/lib.rs` containing only a `//!` crate-level doc
  comment: "Pure domain types, config schema, error enum. Zero I/O. Zero async.
  No tokio, no sqlx, no network." — approximately 3 lines total.
- Add `"crates/anvilml-core"` to the `members` array in the root `Cargo.toml`.
- Verify `cargo build -p anvilml-core` exits 0.

### Out of Scope
None. This task's `defers_to` field is empty (`[]`); no scope is deferred.

## Existing Codebase Assessment

No prior source exists under `crates/` — the directory does not yet exist on disk.
Phase 1 Group A (P1-A1) has established the workspace root `Cargo.toml` with
`members = ["backend"]`, the `[workspace.package]` block (version 0.1.0, edition 2024,
rust-version 1.96.0), and the `rust-toolchain.toml` pin. The `backend/` crate exists
and compiles with `clap` and `tokio` dependencies.

This task establishes the baseline crate directory structure (`crates/<name>/Cargo.toml`
+ `src/lib.rs`) and the convention of workspace-inherited version/edition that all
subsequent crate stub tasks will follow. No external API shapes, test patterns, or
logging conventions apply at this stage since no functions or types are being written.

## Resolved Dependencies

None. This task introduces no external dependencies — `anvilml-core` has zero
dependencies per the crate dependency graph (`ANVILML_DESIGN.md §3.2`).

## Approach

1. **Create directory structure.**
   ```bash
   mkdir -p crates/anvilml-core/src
   ```
   This creates both `crates/` and `crates/anvilml-core/src/` in one call.

2. **Write `crates/anvilml-core/Cargo.toml`.**
   The file contains:
   ```toml
   [package]
   name = "anvilml-core"
   version.workspace = true
   edition.workspace = true
   rust-version.workspace = true
   ```
   No `[dependencies]` section. No features. Version and edition are inherited from
   the workspace root's `[workspace.package]` block (established by P1-A1). This
   matches the convention used by `backend/Cargo.toml` (the only existing crate
   `Cargo.toml` in the repo at this point).

3. **Write `crates/anvilml-core/src/lib.rs`.**
   The file contains exactly three lines:
   ```rust
   //! anvilml-core — Pure domain types, config schema, error enum.
   //! Zero I/O. Zero async. No tokio, no sqlx, no network.
   ```
   This satisfies the `lib.rs` discipline (`ANVILML_DESIGN.md §4.1`, `ARCHITECTURE.md §2`,
   `FORGE_AGENT_RULES.md §12.3`): only a `//!` crate-level doc comment, no `pub mod`,
   no `pub use`, no implementation code. The file will be 3 lines, well under the 80-line
   hard cap.

4. **Add `crates/anvilml-core` to root `Cargo.toml` members.**
   Modify the `members` array from `["backend"]` to `["backend", "crates/anvilml-core"]`.
   This is safe because the directory now exists — Cargo would error if listed before
   the directory is created (`TASKS_PHASE001.md` known constraints).

5. **Verify compilation.**
   Run `cargo build -p anvilml-core` and confirm exit code 0.

## Public API Surface

None. This task creates only an empty crate stub with a doc comment — no `pub` items
are declared. Submodules (`pub mod config`, `pub mod error`, etc.) are added by later
tasks (Phase 2/3).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-core/Cargo.toml` | Package manifest with workspace-inherited version/edition, zero deps |
| CREATE | `crates/anvilml-core/src/lib.rs` | Crate-level doc comment only (~3 lines) |
| MODIFY | `Cargo.toml` (root) | Add `"crates/anvilml-core"` to workspace `members` array |

## Tests

None. This task introduces no source code, no functions, and no types — only a
doc-commented stub crate and a manifest entry. There is nothing to test beyond
compilation, which is covered by the acceptance criterion.

## CI Impact

No CI changes required. The workspace-level `cargo build --workspace` and
`cargo test --workspace` commands (used in CI jobs defined by P1-E1) will
automatically pick up the new crate member. No new CI job or gate is needed.

## Platform Considerations

None identified. This task writes only a TOML manifest and a single-line Rust
doc comment — no platform-specific code, no `#[cfg]` guards, no path separators
or line-ending handling beyond what the workspace's `.gitattributes` already
enforces (LF for `*.toml` and `*.rs`). The Windows cross-check in ENVIRONMENT.md
§7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Root `Cargo.toml` `members` array syntax error when adding the new path | Low | High — would break the entire workspace build | Edit carefully: append `"crates/anvilml-core"` inside the existing array, preserving the trailing comma and closing bracket. Verify with `cargo build -p anvilml-core` immediately after. |
| `lib.rs` exceeds 80-line cap due to accidental submodule declarations | Low | Low — would fail the `lib.rs` discipline check | Only write the two-line doc comment. Do not add `pub mod` declarations — those belong to Phase 2/3 tasks that introduce actual types. |

## Acceptance Criteria

- [ ] `cargo build -p anvilml-core` exits 0
