# Plan Report: P1-A1

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P1-A1                                       |
| Phase       | 001 — Walking Skeleton                      |
| Description | anvilml-server: AppState struct             |
| Depends on  | P0-*, none (Phase 000 prerequisites assumed complete) |
| Project     | anvilml                                     |
| Planned at  | 2026-06-14T07:42:00Z                        |
| Attempt     | 1                                           |

## Objective

Create `AppState` — the shared state struct that all HTTP handlers will receive via axum's `State` extractor. It holds `start_time: std::time::Instant` for computing server uptime and `version: String` from `CARGO_PKG_VERSION`. This struct is the foundation that P1-A2 (health handler) and P1-A3 (build_router) build on. The observable state after completion: `cargo test -p anvilml-server` exits 0, confirming the struct compiles, derives correctly, and passes unit tests.

## Scope

### In Scope
- **CREATE** `crates/anvilml-server/src/state.rs` — `pub struct AppState` with `start_time` and `version` fields, `pub fn new()`, `Clone` derive, and `///` doc comment.
- **MODIFY** `crates/anvilml-server/src/lib.rs` — declare `pub mod state;` and `pub use state::AppState;`.
- **MODIFY** `crates/anvilml-server/Cargo.toml` — add `serde_json = { workspace = true }` under `[dev-dependencies]`.
- **CREATE** `crates/anvilml-server/tests/state_tests.rs` — tests verifying `AppState::new()`, `Clone` implementation, and field values.
- Bump `anvilml-server` crate version in `crates/anvilml-server/Cargo.toml` (patch bump).

### Out of Scope
- Health handler implementation (P1-A2).
- Router wiring (P1-A3).
- Any integration tests requiring a running server.
- Any changes to other crates.

## Existing Codebase Assessment

No prior source exists in `crates/anvilml-server/src/` beyond a stub `lib.rs` containing a single `#[allow(dead_code)] pub fn stub() {}` and a crate-level doc comment. This task establishes the baseline patterns for the server crate: module structure (separate file per concern), `pub mod` / `pub use` re-exports in `lib.rs`, and the convention that `lib.rs` contains no implementation code. The crate already declares path dependencies on `anvilml-core`, `anvilml-hardware`, `anvilml-ipc`, `anvilml-scheduler`, and `anvilml-worker`, plus workspace deps `axum`, `tower-http`, and `tracing`. The workspace `Cargo.toml` provides `serde_json = "1.0.150"` as a workspace dependency, which this task will reference as a dev-dependency. The `mock-hardware` feature forwarding is already declared in the crate's `[features]` section.

## Resolved Dependencies

| Type   | Name       | Version verified | MCP source     | Feature flags confirmed |
|--------|------------|-----------------|----------------|------------------------|
| crate  | serde_json | 1.0.150         | workspace Cargo.toml | n/a (dev-dep, no features) |

Note: `serde_json` is already declared in the workspace `[workspace.dependencies]` section at version 1.0.150. This task references it via `workspace = true` under `[dev-dependencies]` in the anvilml-server crate's `Cargo.toml`. No new external crate is introduced — only a dev-dependency reference.

## Approach

1. **Create `crates/anvilml-server/src/state.rs`** with the following content:
   - `/// AppState holds shared server state accessible to all HTTP handlers.`
   - `#[derive(Clone)]` on `pub struct AppState` (axum's `State` extractor requires `Clone`).
   - Fields: `start_time: std::time::Instant` and `version: String`.
   - `/// Create a new AppState with the given server version.`
   - `pub fn new(version: impl Into<String>) -> Self` that stores `std::time::Instant::now()` and the version string.
   - No logging is required at this stage — logging becomes mandatory in Phase 002+ when lifecycle events (bind address, shutdown) are introduced.

2. **Modify `crates/anvilml-server/src/lib.rs`**:
   - Replace the `#[allow(dead_code)] pub fn stub() {}` line with `pub mod state;` and `pub use state::AppState;`.
   - Keep the existing `//!` crate-level doc comment unchanged.
   - Result: `lib.rs` will contain only the crate-level doc comment, `pub mod state;`, and `pub use state::AppState;` — well under the 80-line limit, following the `lib.rs` discipline rule (§12.3 of FORGE_AGENT_RULES).

3. **Modify `crates/anvilml-server/Cargo.toml`**:
   - Add a `[dev-dependencies]` section with `serde_json = { workspace = true }`. This is needed so the test can construct `AppState` and verify its fields via JSON round-trip if desired (though the primary test will use direct field access).
   - The workspace already provides `serde_json = "1.0.150"`; no version pinning is needed.

4. **Create `crates/anvilml-server/tests/state_tests.rs`** with three tests:
   - **`test_app_state_new`**: Calls `AppState::new("0.1.0")` and verifies `start_time` is non-zero (by computing `Instant::now() - state.start_time` and asserting it is less than 1 second) and `version` equals `"0.1.0"`.
   - **`test_app_state_clone`**: Clones the `AppState` and verifies the cloned `version` field matches the original. (`Instant` does not compare equal across clones, so we only verify the `String` field.)
   - **`test_app_state_version_from_env`**: Uses `env!("CARGO_PKG_VERSION")` to get the crate version and passes it to `AppState::new()`, then verifies the stored version matches. This confirms the constructor accepts `&'static str` via `impl Into<String>`.

5. **Bump `anvilml-server` version** in `crates/anvilml-server/Cargo.toml`:
   - Read current version from workspace (currently `"0.1.0"` via `version.workspace = true`).
   - Bump to `0.1.1` by setting `version = "0.1.1"` explicitly (since workspace version is read-only, we override at the crate level for the patch bump).
   - Per §12 of ENVIRONMENT.md: only the patch version changes; workspace version stays at `0.1.0`.

## Public API Surface

| Item | Type | Module Path | Signature / Definition |
|------|------|-------------|----------------------|
| `AppState` | struct | `anvilml_server::AppState` (re-exported from `state`) | `pub struct AppState { start_time: std::time::Instant, version: String }` with `#[derive(Clone)]` |
| `AppState::new` | fn | `anvilml_server::AppState::new` | `pub fn new(version: impl Into<String>) -> Self` |

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `crates/anvilml-server/src/state.rs` | AppState struct with new() constructor and Clone derive |
| MODIFY | `crates/anvilml-server/src/lib.rs` | Declare `pub mod state;` and `pub use state::AppState;`, remove stub |
| MODIFY | `crates/anvilml-server/Cargo.toml` | Add `[dev-dependencies]` with `serde_json = { workspace = true }`; bump version to 0.1.1 |
| CREATE | `crates/anvilml-server/tests/state_tests.rs` | Three unit tests for AppState::new(), Clone, and version field |

## Tests

| Test File | Test Name | What It Verifies | Acceptance Command |
|-----------|-----------|-----------------|-------------------|
| `crates/anvilml-server/tests/state_tests.rs` | `test_app_state_new` | `AppState::new()` sets `start_time` to a recent instant and stores the version string correctly | `cargo test -p anvilml-server -- test_app_state_new` exits 0 |
| `crates/anvilml-server/tests/state_tests.rs` | `test_app_state_clone` | `Clone` derive works — cloned `version` matches original | `cargo test -p anvilml-server -- test_app_state_clone` exits 0 |
| `crates/anvilml-server/tests/state_tests.rs` | `test_app_state_version_from_env` | `new()` accepts `&'static str` from `CARGO_PKG_VERSION` and stores it | `cargo test -p anvilml-server -- test_app_state_version_from_env` exits 0 |

## CI Impact

No CI changes required. The new test file lives under `crates/anvilml-server/tests/` which is automatically picked up by `cargo test --workspace --features mock-hardware`. No new file types, gates, or test modules are introduced that would require CI configuration changes.

## Platform Considerations

None identified. `std::time::Instant` and `String` are platform-neutral. `Instant::now()` works identically on Linux and Windows. The `Clone` derive on this struct is also platform-neutral. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `Instant` does not implement `PartialEq` or `Eq`, so direct comparison in tests will fail to compile. | High | High | Use `Instant::now() - state.start_time` to compute elapsed duration and assert it is less than 1 second instead of comparing `Instant` values directly. |
| Workspace version bump via `version.workspace = true` means the patch bump requires switching to an explicit `version = "0.1.1"` at the crate level, which deviates from the workspace convention. | Medium | Low | Per §14 of FORGE_AGENT_RULES, the workspace version is read-only. Setting an explicit crate-level version for the patch bump is the correct approach and documented in ENVIRONMENT.md §12. |
| Adding `serde_json` as a dev-dependency may cause a cargo lockfile update that the workspace lockfile treats as a drift. | Low | Low | `serde_json` is already a workspace dependency at 1.0.150; referencing it via `workspace = true` ensures version consistency. The lockfile update is deterministic and expected. |

## Acceptance Criteria

- [ ] `cargo test -p anvilml-server --features mock-hardware` exits 0 with all 3 tests passing
- [ ] `head -1 .forge/reports/P1-A1_plan.md` prints `# Plan Report: P1-A1`
- [ ] `grep "^## " .forge/reports/P1-A1_plan.md` shows 11 section headings
- [ ] `wc -l .forge/reports/P1-A1_plan.md` returns a value greater than 40
- [ ] `crates/anvilml-server/src/lib.rs` contains `pub mod state;` and `pub use state::AppState;`
- [ ] `crates/anvilml-server/src/state.rs` exists and defines `pub struct AppState` with `Clone` derive
- [ ] `crates/anvilml-server/Cargo.toml` contains `[dev-dependencies]` section with `serde_json`
- [ ] `crates/anvilml-server/tests/state_tests.rs` exists with three test functions
