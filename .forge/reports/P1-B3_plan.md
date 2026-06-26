# Plan Report: P1-B3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P1-B3                                       |
| Phase       | 001 — Repository Scaffold                   |
| Description | anvilml-registry, anvilml-artifacts: empty crate stubs |
| Depends on  | P1-B1                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-26T12:35:00Z                        |
| Attempt     | 1                                           |

## Objective

Create two new Rust crates (`anvilml-registry` and `anvilml-artifacts`) as empty, doc-comment-only stubs in the correct dependency order within the AnvilML workspace. Each crate depends only on `anvilml-core` (path dependency), has a `lib.rs` containing only a crate-level `//!` doc comment (≤ 80 lines), and is registered in the root `Cargo.toml` workspace members. The acceptance criterion is that `cargo build -p anvilml-registry -p anvilml-artifacts` exits 0.

## Scope

### In Scope
- Create `crates/anvilml-registry/Cargo.toml` with workspace-inherited version/edition, path dependency on `anvilml-core`, no `sqlx` dependency.
- Create `crates/anvilml-registry/src/lib.rs` with the crate-level doc comment: "Model scanner + SQLite persistence. Never caches model file contents in memory."
- Create `crates/anvilml-artifacts/Cargo.toml` with workspace-inherited version/edition, path dependency on `anvilml-core`, no `sqlx` dependency.
- Create `crates/anvilml-artifacts/src/lib.rs` with the crate-level doc comment: "Content-addressed PNG artifact storage."
- Add both crate paths to root `Cargo.toml` `members` array.

### Out of Scope
None. `defers_to (from JSON): []` — this task must implement its full scope with no deferrals.

## Existing Codebase Assessment

No prior source exists for either crate — both `crates/anvilml-registry/` and `crates/anvilml-artifacts/` do not yet exist on disk. This task establishes the baseline patterns for these two crates.

The established patterns, observed from the two already-created crates (`anvilml-core` and `anvilml-hardware`):
- **Cargo.toml**: `[package]` block with `name`, `version.workspace = true`, `edition.workspace = true`, `rust-version.workspace = true`, followed by `[dependencies]` with path-only dependencies, and optionally `[features]`.
- **lib.rs**: A single `//!` crate-level doc comment describing what the crate owns and its hard constraints. No submodule declarations, no implementation code. Well under the 80-line cap.
- **Workspace membership**: Each crate adds its path to the root `Cargo.toml` `members` array. Current members are `["backend", "crates/anvilml-core", "crates/anvilml-hardware"]`.

No gap between the design doc and current source affects this approach — the design doc (§3.2 dependency graph) and the task context both specify the same minimal stub shape.

## Resolved Dependencies

| Type   | Name          | Version verified | MCP source     | Feature flags confirmed |
|--------|---------------|-----------------|----------------|------------------------|
| crate  | anvilml-core  | 0.1.0 (workspace) | N/A (path dep) | n/a                    |

No new external dependencies are introduced by this task. Both crates depend only on `anvilml-core` via a workspace path dependency, which already exists and compiles.

## Approach

1. **Create `crates/anvilml-registry/Cargo.toml`**: Write a `[package]` block with `name = "anvilml-registry"`, `version.workspace = true`, `edition.workspace = true`, `rust-version.workspace = true`. Add a `[dependencies]` section with `anvilml-core = { path = "../anvilml-core" }`. No `[features]` section (no feature flag needed at stub stage; `mock-hardware` is on `anvilml-hardware`, not here).

2. **Create `crates/anvilml-registry/src/lib.rs`**: Write a single `//!` crate-level doc comment: `//! Model scanner + SQLite persistence. Never caches model file contents in memory.` No other content. Total: 1 line, well under 80-line cap.

3. **Create `crates/anvilml-artifacts/Cargo.toml`**: Same structure as step 1 — `[package]` with `name = "anvilml-artifacts"`, workspace-inherited version/edition/rust-version, and `[dependencies]` with `anvilml-core = { path = "../anvilml-core" }`. No `[features]` section.

4. **Create `crates/anvilml-artifacts/src/lib.rs`**: Write a single `//!` crate-level doc comment: `//! Content-addressed PNG artifact storage.` No other content. Total: 1 line, well under 80-line cap.

5. **Update root `Cargo.toml`**: Append `"crates/anvilml-registry"` and `"crates/anvilml-artifacts"` to the existing `members` array. Current value: `["backend", "crates/anvilml-core", "crates/anvilml-hardware"]`. New value: `["backend", "crates/anvilml-core", "crates/anvilml-hardware", "crates/anvilml-registry", "crates/anvilml-artifacts"]`.

6. **Verify build**: Run `cargo build -p anvilml-registry -p anvilml-artifacts` and confirm exit 0. This exercises the full dependency resolution and compilation of both new crates against `anvilml-core`.

No external API names, version numbers, or feature flags need MCP verification — both crates depend only on an existing path dependency with no external crates.

## Public API Surface

None. Both `lib.rs` files contain only crate-level doc comments (`//!`). No `pub` items are declared.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-registry/Cargo.toml` | Package manifest: workspace-inherited version, path dep on `anvilml-core` |
| CREATE | `crates/anvilml-registry/src/lib.rs` | Crate stub: doc comment only (≤ 80 lines) |
| CREATE | `crates/anvilml-artifacts/Cargo.toml` | Package manifest: workspace-inherited version, path dep on `anvilml-core` |
| CREATE | `crates/anvilml-artifacts/src/lib.rs` | Crate stub: doc comment only (≤ 80 lines) |
| MODIFY | `Cargo.toml` (root) | Add both crate paths to workspace `members` array |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-registry/tests/stub_tests.rs` | `test_registry_crate_compiles` | The `anvilml-registry` crate compiles and its public API is callable (currently empty) | Workspace builds | None | Build succeeds, no compile errors | `cargo build -p anvilml-registry` exits 0 |
| `crates/anvilml-artifacts/tests/stub_tests.rs` | `test_artifacts_crate_compiles` | The `anvilml-artifacts` crate compiles and its public API is callable (currently empty) | Workspace builds | None | Build succeeds, no compile errors | `cargo build -p anvilml-artifacts` exits 0 |

Note: Because both crates are empty stubs with no `pub` items, the acceptance criterion is the build command itself (`cargo build -p anvilml-registry -p anvilml-artifacts`), which compiles both crates as a single invocation. The individual test files above are thin wrappers around the same build verification.

## CI Impact

No CI changes required. The new crates are part of the workspace, so existing CI jobs (`rust-linux`, `rust-windows`) that run `cargo test --workspace --features mock-hardware` will automatically pick them up once they are added as workspace members. No new CI jobs or steps are needed.

## Platform Considerations

None identified. The task introduces no platform-specific code — both crates are pure Rust stubs with no `#[cfg]` guards, no I/O, no filesystem access, and no async code. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| Root `Cargo.toml` `members` array ordering causes Cargo to resolve a crate before its dependency | Low | High | Add crates in dependency order: `anvilml-registry` and `anvilml-artifacts` both depend only on `anvilml-core`, which is already in `members` at an earlier position. This is the same pattern used by P1-B1 (core) → P1-B2 (hardware). |
| `lib.rs` doc comment text does not match the exact wording specified in the task context | Low | Low | The task context provides the exact strings verbatim. Copy them directly without paraphrasing. |

## Acceptance Criteria

- [ ] `cargo build -p anvilml-registry -p anvilml-artifacts` exits 0
