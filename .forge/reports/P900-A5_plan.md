# Plan Report: P900-A5

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P900-A5                                           |
| Phase       | 900 — Logging Retrofit                            |
| Description | anvilml-core: retrofit DEBUG resolved config log to config_load.rs |
| Depends on  | P7-C1 (tracing in workspace deps)                 |
| Project     | anvilml                                           |
| Planned at  | 2026-06-06T00:35:00Z                              |
| Attempt     | 1                                                 |

## Objective

Add a single `tracing::debug!` call to `load_config()` in `crates/anvilml-core/src/config_load.rs` that logs the final resolved configuration values after all four override layers have been applied. This satisfies FORGE_AGENT_RULES §11.1 (general instrumentation obligation) and §11.5 (mandatory DEBUG log points for decision-observable code paths). No logic changes are made.

## Scope

### In Scope
- Add `tracing = { workspace = true }` to `crates/anvilml-core/Cargo.toml` (first-time dep for this crate; tracing is already in `[workspace.dependencies]` as "0.1.44" per P7-C1).
- Add one `tracing::debug!` call at the end of `load_config()` (just before `Ok(config)`), logging: `host`, `port`, `db_path`, and `frontend_mode`.
- No changes to any other file, function, test, or logic branch.

### Out of Scope
- Adding log calls for other fields (e.g. `venv_path`, `seeds_path`) — excluded per task instructions as they may contain user-specified paths treated as sensitive.
- Modifying `config.rs`, `error.rs`, or any other module in `anvilml-core`.
- Adding tests — the existing test suite must pass without modification.
- Any changes to `backend/`, `crates/anvilml-hardware/`, `crates/anvilml-registry/`, or other crates.

## Approach

1. **Add tracing dependency** to `crates/anvilml-core/Cargo.toml`:
   - Append `tracing = { workspace = true }` under `[dependencies]`.
   - This uses the existing workspace-level version `"0.1.44"` (confirmed in root `Cargo.toml` line 37).

2. **Add DEBUG log call** to `crates/anvilml-core/src/config_load.rs`:
   - Insert after line 255 (`Ok(config)`) and before the closing brace of the `Ok(...)` variant, i.e. immediately before the return:
     ```rust
     tracing::debug!(host = %cfg.host, port = cfg.port, db_path = %cfg.db_path.display(), frontend_mode = ?cfg.frontend.mode, "config resolved");
     ```
   - Wait — the variable is named `config`, not `cfg`. The correct call:
     ```rust
     tracing::debug!(host = %config.host, port = config.port, db_path = %config.db_path.display(), frontend_mode = ?config.frontend.mode, "config resolved");
     ```
   - Structured field notation per §11.6: `=` for all fields (no `%` or `?` prefix in the macro call itself; `tracing` handles formatting via the type). Actually, per §11.6 and the task spec: use `=%` for displayable strings (`host`, `db_path`), plain `=` for integers (`port`), and `=?` for the enum option (`frontend_mode`). The exact call:
     ```rust
     tracing::debug!(host = %config.host, port = config.port, db_path = %config.db_path.display(), frontend_mode = ?config.frontend.mode, "config resolved");
     ```

3. **Verify** that `cargo test -p anvilml-core -- config_load` exits 0.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `crates/anvilml-core/Cargo.toml` | Add `tracing = { workspace = true }` to `[dependencies]` |
| Modify | `crates/anvilml-core/src/config_load.rs` | Add one `tracing::debug!` call before return in `load_config()` |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-core/src/config_load.rs` (inline tests) | `env_overrides_toml` | Existing test continues to pass — no regression |
| `crates/anvilml-core/src/config_load.rs` (inline tests) | `override_beats_env` | Existing test continues to pass — no regression |
| `crates/anvilml-core/src/config_load.rs` (inline tests) | `missing_toml_fallback` | Existing test continues to pass — no regression |
| `crates/anvilml-core/src/config_load.rs` (inline tests) | `env_nested_field` | Existing test continues to pass — no regression |

No new test files are added. The task makes no logic changes, so existing assertions remain valid.

## CI Impact

No CI workflow files are modified. The only change is adding an optional `tracing` dependency (which is already in the workspace) and a DEBUG-level log call that is invisible at the default INFO level. All existing CI gates (`cargo test --workspace --features mock-hardware`, `cargo clippy`, format checks, platform cross-checks) should continue to pass without modification.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `tracing` not yet in workspace deps (P7-C1 not executed) | Low | High — build fails | Task prerequisites confirm P7-C1 is complete; if missing, STOP and document as blocker |
| `tracing::debug!` macro requires `tracing` crate at compile time even when DEBUG is off | None | None — Rust dependencies are compile-time only, runtime cost is zero when not at DEBUG level | N/A |
| Adding dependency causes transitive dependency drift in the workspace lockfile | Low | Medium — `Cargo.lock` changes | Run `cargo check -p anvilml-core --features mock-hardware` to verify; lockfile update is expected and harmless |
| Log call placement inside a `return` statement could be syntactically problematic | None | Low — just place as a statement before the final `Ok(config)` return | Place on its own line between the overrides block and `Ok(config)` |

## Acceptance Criteria

- [ ] `tracing = { workspace = true }` added to `crates/anvilml-core/Cargo.toml`
- [ ] `tracing::debug!(host = %config.host, port = config.port, db_path = %config.db_path.display(), frontend_mode = ?config.frontend.mode, "config resolved")` present in `load_config()` before the return
- [ ] No logic changes — function signature, control flow, and test assertions unchanged
- [ ] `cargo test -p anvilml-core -- config_load` exits 0
- [ ] `cargo check -p anvilml-core --features mock-hardware` exits 0
