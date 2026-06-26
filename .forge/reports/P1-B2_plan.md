# Plan Report: P1-B2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P1-B2                                       |
| Phase       | 001 — Repository Scaffold                   |
| Description | anvilml-hardware: empty crate stub + mock-hardware feature decl |
| Depends on  | P1-B1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-26T14:12:00Z                        |
| Attempt     | 1                                           |

## Objective

Create the `anvilml-hardware` crate as an empty, doc-commented stub with the `mock-hardware` feature declared at its point of origin. This establishes the hardware-detection crate in the dependency graph so that every later crate can forward the `mock-hardware` flag (`anvilml-worker`, `anvilml-scheduler`, `anvilml-server`, `backend`) without a forward reference to a non-existent feature. At completion, both `cargo build -p anvilml-hardware` and `cargo build -p anvilml-hardware --features mock-hardware` exit 0.

## Scope

### In Scope
- Create `crates/anvilml-hardware/Cargo.toml` with: package name `anvilml-hardware`, workspace-inherited version/edition/rust-version, path dependency on `anvilml-core`, and `[features] mock-hardware = []`.
- Create `crates/anvilml-hardware/src/lib.rs` with a `//!` crate-level doc comment only: "GPU/CPU detection. Never panics on missing driver. Always returns >=1 CPU device." — no submodules, no implementation, no tests yet. File stays well under 80 lines.
- Modify root `Cargo.toml` to add `"crates/anvilml-hardware"` to the `members` array.

### Out of Scope
None. `defers_to (from JSON): []` — this task has an empty defers_to field and implements its full scope. No functionality is deferred to any other task.

## Existing Codebase Assessment

No prior source exists for `anvilml-hardware` — the directory does not exist on disk. The only existing crate is `anvilml-core`, which follows the established stub pattern: a 5-line `Cargo.toml` using workspace-inherited metadata with zero dependencies, and a 2-line `lib.rs` containing only a `//!` crate-level doc comment. This is the baseline pattern that `anvilml-hardware` will replicate, with the addition of a path dependency on `anvilml-core` and the `[features] mock-hardware = []` declaration. The root `Cargo.toml` workspace currently lists `"backend"` and `"crates/anvilml-core"` in its `members` array; crates are added incrementally as they are created (never batched up-front, per TASKS_PHASE001.md known constraints).

## Resolved Dependencies

No external (crates.io / PyPI / npm) dependencies are introduced by this task. The only dependency is the workspace-internal path dependency on `anvilml-core`, which already exists and compiles.

| Type   | Name    | Version verified | MCP source     | Feature flags confirmed |
|--------|---------|-----------------|----------------|------------------------|
| path   | anvilml-core | 0.1.0 (workspace-inherited) | Cargo.toml read | none |

## Approach

1. **Create `crates/anvilml-hardware/` directory and `src/` subdirectory.**
   - These directories do not exist yet; create them as empty containers before writing files.

2. **Write `crates/anvilml-hardware/Cargo.toml`.**
   - Package name: `anvilml-hardware`
   - Version: `version.workspace = true` (inherits `0.1.0` from root)
   - Edition: `edition.workspace = true` (inherits `2024`)
   - Rust version: `rust-version.workspace = true` (inherits `1.96.0`)
   - Dependencies: `[dependencies]` section with `anvilml-core = { path = "../anvilml-core" }` — path dependency matching the crate dependency graph in `ANVILML_DESIGN.md §3.2`.
   - Features: `[features]` section with `mock-hardware = []` — intentionally empty at this phase, exists purely so later crates can forward it without a forward reference.
   - This follows the exact pattern established by `crates/anvilml-core/Cargo.toml` plus the dependency/feature additions.

3. **Write `crates/anvilml-hardware/src/lib.rs`.**
   - Single `//!` crate-level doc comment: `//! GPU/CPU detection. Never panics on missing driver. Always returns >=1 CPU device.`
   - No `pub mod` declarations, no submodules, no implementation code, no tests.
   - File will be ~2 lines, well under the 80-line hard cap (`ANVILML_DESIGN.md §4.1` / `FORGE_AGENT_RULES.md §13`).

4. **Modify root `Cargo.toml`** to add `"crates/anvilml-hardware"` to the `members` array.
   - Current members: `["backend", "crates/anvilml-core"]`
   - New members: `["backend", "crates/anvilml-core", "crates/anvilml-hardware"]`
   - Append to the existing array line; do not reformat the entire file.

5. **Verify acceptance criteria locally.**
   - Run `cargo build -p anvilml-hardware` — must exit 0.
   - Run `cargo build -p anvilml-hardware --features mock-hardware` — must exit 0.
   - Both commands must succeed; if either fails, diagnose and fix before proceeding.

## Public API Surface

None. This task creates an empty stub crate — no `pub` items, no functions, no types, no traits. The crate will expose nothing until subsequent phases add detection modules (`detect.rs`, `cpu.rs`, `vulkan.rs`, etc.).

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | crates/anvilml-hardware/Cargo.toml | Crate manifest: path dep on anvilml-core, mock-hardware feature decl |
| CREATE | crates/anvilml-hardware/src/lib.rs | Crate-level doc comment only (~2 lines) |
| MODIFY | Cargo.toml | Add "crates/anvilml-hardware" to workspace members array |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| (build) | build_no_feature | `anvilml-hardware` compiles without any features | None | `cargo build -p anvilml-hardware` | Exit 0, no output errors | `cargo build -p anvilml-hardware` exits 0 |
| (build) | build_with_mock_feature | `anvilml-hardware` compiles with `mock-hardware` feature enabled | None | `cargo build -p anvilml-hardware --features mock-hardware` | Exit 0, no output errors | `cargo build -p anvilml-hardware --features mock-hardware` exits 0 |

Note: The acceptance criteria are build-only (no test code is written in this task — the crate is a stub). The build commands themselves serve as the verification. No `#[cfg(test)]` or integration tests are needed for an empty stub crate.

## CI Impact

No CI changes required. The workspace already has `cargo build --workspace --features mock-hardware` in its CI jobs (P1-E1). Adding `anvilml-hardware` to the workspace means it will be built as part of the workspace build automatically — no CI job modification needed.

## Platform Considerations

None identified. The `mock-hardware` feature is declared as `[]` (empty) — no `#[cfg]` guards are introduced. The crate contains no platform-specific code. The Windows cross-check in `ENVIRONMENT.md §7` is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Root `Cargo.toml` `members` array formatting causes Cargo to misparse | Low | Medium | Append the new member to the existing array line without changing other formatting; verify with `cargo build -p anvilml-hardware` immediately after. |
| Path dependency on `anvilml-core` is incorrect (wrong relative path) | Low | High | The crate lives at `crates/anvilml-hardware/` and `anvilml-core` at `crates/anvilml-core/`, so the relative path from hardware to core is `../anvilml-core` — verified by the existing workspace layout. |

## Acceptance Criteria

- [ ] `cargo build -p anvilml-hardware` exits 0
- [ ] `cargo build -p anvilml-hardware --features mock-hardware` exits 0
