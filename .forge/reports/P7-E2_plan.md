# Plan Report: P7-E2

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P7-E2                                       |
| Phase       | 007 — WebSocket Event Stream                |
| Description | anvilml-core: migrate toml dependency from 0.8.x to 1.x |
| Depends on  | P7-C1, P7-E1                                |
| Project     | anvilml                                     |
| Planned at  | 2026-06-05T00:48:00Z                        |
| Attempt     | 1                                           |

## Objective

Migrate the `toml` crate dependency from version 0.8.x to 1.x across the AnvilML workspace, updating all three breaking API call sites that changed between these major versions.

## Scope

### In Scope
- Bump `toml` from `"0.8"` to `"1"` in `[workspace.dependencies]` at root `Cargo.toml`.
- Update `ConfigError::Toml` variant type and its `From<toml::de::Error>` impl in `config_load.rs` to use the unified `toml::Error` type exposed at the crate root in 1.x.
- Update `toml::to_string_pretty` call in `config.rs` (test `test_toml_roundtrip`) to the new path `toml::ser::to_string_pretty`.
- Update `toml::to_string_pretty` call in `backend/tests/config_reference.rs` (test `test_toml_key_set_matches_default`) to the new path.
- Verify `toml::Value` and `toml::from_str` usage in `config_reference.rs` compiles unchanged (both are available at the crate root in 1.x).
- All tests pass: `cargo test --workspace --features mock-hardware`.
- Lint clean: `cargo clippy --workspace -- -D warnings`.

### Out of Scope
- Any changes to `anvilml-core/src/config.rs` non-test code (the config types themselves do not depend on toml internals).
- Changes to other crates that transitively depend on toml but have no direct call sites affected by the 0.8→1.x API change.
- Upgrading any other dependency.
- Modifying CI workflow files.
- Adding new tests beyond verifying existing ones pass.

## Approach

1. **Bump version in root Cargo.toml.** Change `toml = "0.8"` to `toml = "1"` in `[workspace.dependencies]`. The per-crate dependency (`crates/anvilml-core/Cargo.toml`) already uses `{ workspace = true }`, so no per-crate Cargo.toml edits are needed.

2. **Update config_load.rs error type.** In `ConfigError::Toml` variant, change the inner type from `toml::de::Error` to `toml::Error`. Update the `From<toml::de::Error>` impl to `From<toml::Error>`. The `Display` and `source()` implementations remain unchanged since both error types implement `std::error::Error + Display`.

3. **Update config.rs test serialization call.** In the `test_toml_roundtrip` test, change `toml::to_string_pretty(&config)` to `toml::ser::to_string_pretty(&config)`. The `toml::from_str` call is unchanged (it is available at the crate root in 1.x).

4. **Update config_reference.rs serialization calls.** In the drift guard test, change both occurrences of `toml::to_string_pretty` to `toml::ser::to_string_pretty`. The `toml::Value` type and `toml::from_str` usage remain at the crate root path and compile unchanged.

5. **Verify build + tests.** Run `cargo test --workspace --features mock-hardware` and `cargo clippy --workspace -- -D warnings`. Fix any compilation errors that arise from unforeseen API differences.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| Modify | `Cargo.toml` | Bump `toml` from `"0.8"` to `"1"` in `[workspace.dependencies]` |
| Modify | `crates/anvilml-core/src/config_load.rs` | Update `ConfigError::Toml` variant type and `From` impl to use `toml::Error` |
| Modify | `crates/anvilml-core/src/config.rs` | Update `toml::to_string_pretty` → `toml::ser::to_string_pretty` in test |
| Modify | `backend/tests/config_reference.rs` | Update `toml::to_string_pretty` → `toml::ser::to_string_pretty` (2 occurrences) |

## Tests

| Test File | Test Name | What It Verifies |
|-----------|-----------|------------------|
| `crates/anvilml-core/src/config.rs` | `test_toml_roundtrip` | ServerConfig serializes and deserializes correctly via toml 1.x APIs |
| `crates/anvilml-core/src/config.rs` | `test_empty_toml_uses_defaults` | Empty TOML parses with serde defaults (uses `toml::from_str`) |
| `crates/anvilml-core/src/config_load.rs` | `env_overrides_toml` | Config loading via `toml::from_str` works in the layer chain |
| `crates/anvilml-core/src/config_load.rs` | `override_beats_env` | Config loading via `toml::from_str` works in the layer chain |
| `crates/anvilml-core/src/config_load.rs` | `missing_toml_fallback` | Config loading path (no toml parsing needed) |
| `crates/anvilml-core/src/config_load.rs` | `env_nested_field` | Env var parsing (no toml parsing needed) |
| `backend/tests/config_reference.rs` | `test_toml_key_set_matches_default` | Drift guard: `anvilml.toml` key-set matches `ServerConfig::default()` using `toml::Value`, `toml::from_str`, and `toml::ser::to_string_pretty` |

## CI Impact

No CI workflow changes. The `rust` job runs `cargo clippy --workspace --features mock-hardware -D warnings` and `cargo test --workspace --features mock-hardware`, both of which will exercise the updated toml dependency. No new CI jobs or steps are required.

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| `toml::de::Error` path changed in 1.x (e.g., unified to `toml::Error`) | The task description explicitly calls this out; we update the variant type and `From` impl accordingly. If the exact path differs, cargo compilation errors will surface immediately and be fixed before writing the report. |
| `toml::to_string_pretty` path differs from expected (`toml::ser::to_string_pretty`) | The docs.rs output confirms the function exists at both `toml::to_string_pretty` (crate root) and `toml::ser::to_string_pretty`. We use `toml::ser::to_string_pretty` as specified in the task description; if compilation fails, we fall back to the crate-root path. |
| `toml::Value` API changed (e.g., restructured variants) | The docs.rs output confirms `toml::Value` is still an enum at the crate root with standard variants (String, Integer, Float, Boolean, Datetime, Array, Table). Usage in `config_reference.rs` pattern-matches these variants and will continue to compile. |
| `toml::from_str` return type changed | The docs.rs output confirms `toml::from_str` is a crate-root function accepting any `Deserialize` type. The return error type will be consistent with the unified `toml::Error`. |
| `Cargo.lock` regeneration causes unexpected transitive dependency changes | Bumping from 0.8 to 1.x may update sub-crates (e.g., `indexmap`, `serde_spanned`). This is expected and harmless; `cargo test` will catch any incompatibility. |

## Acceptance Criteria

- [ ] `cargo build --workspace --features mock-hardware` exits 0
- [ ] `cargo test --workspace --features mock-hardware` exits 0 (all tests pass, including config roundtrip and drift guard)
- [ ] `cargo clippy --workspace -- -D warnings` exits 0
- [ ] Root `Cargo.toml` `[workspace.dependencies]` contains `toml = "1"`
- [ ] No remaining references to `toml::de::Error` in source code (replaced with `toml::Error`)
- [ ] No remaining references to old `toml::to_string_pretty` path outside the crate root (using `toml::ser::to_string_pretty`)
