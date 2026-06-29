# Plan Report: P6-A9

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P6-A9                                       |
| Phase       | 6 — Model Registry & Artifacts              |
| Description | anvilml-registry: lib.rs re-export pass, 80-line check |
| Depends on  | P6-A8                                       |
| Project     | anvilml                                     |
| Planned at  | 2026-06-29T22:55:00Z                        |
| Attempt     | 1                                           |

## Objective

Verify that `crates/anvilml-registry/src/lib.rs` correctly re-exports all five modules and public items defined in this phase's Group A tasks, and confirm the file stays within the 80-line hard cap. No new code is written — this is a verification pass confirming the existing state meets ANVILML_DESIGN.md §7.1's module layout specification.

## Scope

### In Scope
- Confirm `pub mod` declarations for `db`, `device_store`, `scanner`, `seed_loader`, and `store` are present in `lib.rs`.
- Confirm `pub use` re-exports for `create_pool` (from `db`), `ModelStore` (from `store`), `ModelScanner` (from `scanner`), `DeviceCapabilityStore` (from `device_store`), and `SeedLoader` (from `seed_loader`) are present.
- Verify `wc -l crates/anvilml-registry/src/lib.rs` reports ≤ 80.
- Run `cargo test -p anvilml-registry` (full crate suite) and confirm exit 0.
- Verify the crate-level `//!` doc comment is present (FORGE_AGENT_RULES.md §12.3).

### Out of Scope
defers_to (from JSON): []
None. This task's `defers_to` field is empty — no scope may be deferred. The re-export pass is fully implemented already; this task confirms it. No implementation logic, no new types, no new modules.

## Existing Codebase Assessment

The `anvilml-registry` crate is fully implemented across its five source modules (`db.rs`, `scanner.rs`, `store.rs`, `device_store.rs`, `seed_loader.rs`), each with its own test file under `tests/`. The `lib.rs` at 13 lines already contains:

- A `//!` crate-level doc comment: "Model scanner + SQLite persistence. Never caches model file contents in memory."
- Five `pub mod` declarations: `db`, `device_store`, `scanner`, `seed_loader`, `store` (in alphabetical-ish order).
- Five `pub use` re-exports: `create_pool`, `DeviceCapabilityStore`, `ModelScanner`, `SeedLoader`, `ModelStore`.

All five public items match exactly what ANVILML_DESIGN.md §7.1 specifies. The file is 13 lines, well under the 80-line cap. All five test files exist (`db_tests.rs`, `store_tests.rs`, `scanner_tests.rs`, `device_store_tests.rs`, `seed_loader_tests.rs`). The crate's dependencies (`sqlx`, `tokio`, `anvilml-core`, `chrono`, `sha2`, `digest`, `serde_json`, `tracing`) are already declared in `Cargo.toml` (version 0.1.6).

The established pattern for this project's `lib.rs` files is: `//!` doc comment → blank line → `pub mod` declarations → blank line → `pub use` re-exports. This crate's `lib.rs` follows that pattern exactly. No gap exists between the design doc and current source.

## Resolved Dependencies

None. This task introduces no new dependencies. All crate dependencies are already declared in `Cargo.toml` and were resolved in prior phase tasks.

## Approach

1. **Read the current `lib.rs`** at `crates/anvilml-registry/src/lib.rs` and verify each of the five required `pub mod` declarations is present: `db`, `device_store`, `scanner`, `seed_loader`, `store`. These correspond to the five modules created by P6-A2 (db), P6-A4 (scanner), P6-A3 (store), P6-A5 (device_store), and P6-A6/P6-A7 (seed_loader).

2. **Verify each of the five `pub use` re-exports** matches the public item defined in its source module:
   - `pub use db::create_pool;` — confirmed by `grep '^pub async fn create_pool' src/db.rs` returning one match.
   - `pub use store::ModelStore;` — confirmed by `grep '^pub struct ModelStore' src/store.rs`.
   - `pub use scanner::ModelScanner;` — confirmed by `grep '^pub struct ModelScanner' src/scanner.rs`.
   - `pub use device_store::DeviceCapabilityStore;` — confirmed by `grep '^pub struct DeviceCapabilityStore' src/device_store.rs`.
   - `pub use seed_loader::SeedLoader;` — confirmed by `grep '^pub struct SeedLoader' src/seed_loader.rs`.

3. **Verify line count**: run `wc -l crates/anvilml-registry/src/lib.rs` and confirm the output is ≤ 80. The current file is 13 lines.

4. **Verify crate-level doc comment**: confirm the first non-empty line starts with `//!` and describes the crate's ownership and hard constraints (per FORGE_AGENT_RULES.md §12.3).

5. **Run full test suite**: execute `cargo test -p anvilml-registry` and confirm exit 0. This exercises all five test files and confirms the re-exports are correct (tests that import from `anvilml_registry::` would fail at compile time if any re-export were missing or misnamed).

6. **No changes required**. If all checks pass, the task is complete as-is. If any re-export is missing or misnamed, add the missing `pub use` line (same pattern as existing lines). If the file exceeds 80 lines, remove any non-conforming content (implementation code — which should not be present per §12.3).

## Public API Surface

No new public items are introduced. The existing pub surface of the `anvilml_registry` crate remains unchanged:

| Item | Module Path | Kind |
|------|-------------|------|
| `create_pool` | `anvilml_registry::db::create_pool` | `pub async fn` |
| `ModelStore` | `anvilml_registry::store::ModelStore` | `pub struct` |
| `ModelScanner` | `anvilml_registry::scanner::ModelScanner` | `pub struct` |
| `DeviceCapabilityStore` | `anvilml_registry::device_store::DeviceCapabilityStore` | `pub struct` |
| `SeedLoader` | `anvilml_registry::seed_loader::SeedLoader` | `pub struct` |

All five are re-exported at the crate root via `pub use` in `lib.rs`.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| READ | `crates/anvilml-registry/src/lib.rs` | Verify re-exports and line count |
| READ | `crates/anvilml-registry/Cargo.toml` | Confirm version (0.1.6) — no bump needed since no source files are modified |

No files are created or modified. This is a verification-only task. If any re-export is found missing, the corresponding `pub use` line would be added to `lib.rs`.

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `crates/anvilml-registry/tests/db_tests.rs` | (existing tests) | `create_pool` re-export resolves and works | In-memory SQLite available | N/A | Tests pass | `cargo test -p anvilml-registry --test db_tests` exits 0 |
| `crates/anvilml-registry/tests/store_tests.rs` | (existing tests) | `ModelStore` re-export resolves and CRUD works | In-memory SQLite available | N/A | Tests pass | `cargo test -p anvilml-registry --test store_tests` exits 0 |
| `crates/anvilml-registry/tests/scanner_tests.rs` | (existing tests) | `ModelScanner` re-export resolves and scanning works | Temp directory with model files | N/A | Tests pass | `cargo test -p anvilml-registry --test scanner_tests` exits 0 |
| `crates/anvilml-registry/tests/device_store_tests.rs` | (existing tests) | `DeviceCapabilityStore` re-export resolves and lookup works | In-memory SQLite with seed data | N/A | Tests pass | `cargo test -p anvilml-registry --test device_store_tests` exits 0 |
| `crates/anvilml-registry/tests/seed_loader_tests.rs` | (existing tests) | `SeedLoader` re-export resolves and idempotency works | In-memory SQLite available | N/A | Tests pass | `cargo test -p anvilml-registry --test seed_loader_tests` exits 0 |
| (verification) | lib_reexports_present | All five `pub mod` and `pub use` lines present in lib.rs | File exists | N/A | grep finds all 10 lines | `grep -c 'pub \(mod\|use\)' crates/anvilml-registry/src/lib.rs` returns 10 |
| (verification) | line_count_under_cap | lib.rs is ≤ 80 lines | File exists | N/A | Line count ≤ 80 | `wc -l < crates/anvilml-registry/src/lib.rs` returns ≤ 80 |
| (verification) | full_crate_suite | All tests compile and pass with re-exports | Cargo workspace built | N/A | 0 failures | `cargo test -p anvilml-registry` exits 0 |

## CI Impact

No CI changes required. No new files, no new test modules, no new dependencies, no new file types. The existing `rust-linux` and `rust-windows` CI jobs already run `cargo test --workspace --features mock-hardware`, which includes `anvilml-registry`. No changes to `.github/workflows/ci.yml` or any CI configuration.

## Platform Considerations

None identified. This task operates on a single Rust source file (`lib.rs`) with no platform-specific code, no `#[cfg(unix)]`/`#[cfg(windows)]` guards, and no file-system operations beyond reading the file's line count. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| A prior task left a `pub mod` or `pub use` line missing or misnamed, causing compile-time failure when `cargo test -p anvilml-registry` runs | Low | High | The acceptance command `cargo test -p anvilml-registry` will fail to compile if any re-export is broken. The fix is a single `pub use` line addition — straightforward. |
| The file has grown beyond 80 lines due to accidental implementation code being added to lib.rs | Low | Medium | Step 3 of the approach verifies line count. If exceeded, remove any non-conforming content (implementation code must not be in lib.rs per §12.3). |
| The crate-level `//!` doc comment is missing or incomplete | Low | Low | Step 4 of the approach verifies its presence. If missing, add a one-sentence `//!` comment matching the existing style. |

## Acceptance Criteria

- [ ] `wc -l crates/anvilml-registry/src/lib.rs` reports a number ≤ 80
- [ ] `grep -c 'pub mod' crates/anvilml-registry/src/lib.rs` returns 5
- [ ] `grep -c 'pub use' crates/anvilml-registry/src/lib.rs` returns 5
- [ ] `cargo test -p anvilml-registry` exits 0 (full crate suite)
