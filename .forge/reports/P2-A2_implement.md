# Implementation Report: P2-A2

| Field          | Value                                              |
|----------------|----------------------------------------------------|
| Task ID        | P2-A2                                               |
| Phase          | 002 — Config & Graceful Shutdown                    |
| Description    | anvilml-core: layered config loader (defaults -> toml -> env -> overrides) |
| Project        | anvilml                                             |
| Implemented at | 2026-06-01T07:49:42Z                                |
| Attempt        | 1                                                   |

## Summary

Created `crates/anvilml-core/src/config_load.rs` implementing a layered configuration loader that resolves `ServerConfig` from four precedence levels (lowest to highest): built-in defaults, optional TOML file on disk, `ANVILML_*` environment variables (with double-underscore nesting for sub-fields), and explicit CLI overrides. The module defines `ConfigError` (Io, Toml, EnvParse variants), `ConfigOverrides` (host/port fields), and the public `load_config` function. Updated `lib.rs` to register the module and re-export the public API. Four inline tests verify env-overrides-toml, override-beats-env, missing-toml-fallback, and nested-field env parsing.

## Files Changed

| Action   | Path                              | Description                                          |
|----------|-----------------------------------|------------------------------------------------------|
| CREATE   | crates/anvilml-core/src/config_load.rs | Layered config loader: ConfigError enum, ConfigOverrides struct, load_config function with precedence chain, merge helpers, and 4 inline tests |
| MODIFY   | crates/anvilml-core/src/lib.rs    | Added `pub mod config_load;` and re-exports for `ConfigError`, `ConfigOverrides`, `load_config` |

## Test Results

### Linux unit test run (anvilml-core)

```
running 4 tests
test config_load::tests::env_nested_field ... ok
test config_load::tests::missing_toml_fallback ... ok
test config_load::tests::env_overrides_toml ... ok
test config_load::tests::override_beats_env ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 5 filtered out; finished in 0.00s
```

### Full workspace test run (all crates)

```
running 9 tests
test config::tests::test_device_type_default ... ok
test config::tests::test_default_server_config ... ok
test config::tests::test_model_kind_default ... ok
test config::tests::test_empty_toml_uses_defaults ... ok
test config_load::tests::env_nested_field ... ok
test config_load::tests::missing_toml_fallback ... ok
test config_load::tests::override_beats_env ... ok
test config_load::tests::env_overrides_toml ... ok
test config::tests::test_toml_roundtrip ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

### Windows x86_64-pc-windows-gnu cross-check

```
Finished `dev` profile [optimized + debuginfo] target(s) in 0.78s
```

Zero errors on the windows-gnu target. No platform-specific code was introduced.

## CI Changes

No CI changes made.

## Commit Log

```
A  .forge/reports/P2-A2_plan.md
M  .forge/state/CURRENT_TASK.md
M  .forge/state/state.json
A  crates/anvilml-core/src/config_load.rs
M  crates/anvilml-core/src/lib.rs
```

## Acceptance Criteria — Verification

| Criterion                 | Status | Evidence                        |
|---------------------------|--------|---------------------------------|
| `cargo test -p anvilml-core -- config_load` exits 0 (all four tests pass) | PASS | `cargo test -p anvilml-core -- config_load` — 4 passed; 0 failed |
| `cargo clippy -p anvilml-core` exits 0 with no warnings | PASS | `cargo clippy --workspace --features mock-hardware -- -D warnings` — zero warnings across all crates |
| `crates/anvilml-core/src/config_load.rs` defines `ConfigError`, `ConfigOverrides`, and `load_config` | PASS | File exists with all three public types/functions defined |
| `ConfigOverrides` has fields `host: Option<IpAddr>` and `port: Option<u16>` | PASS | Struct definition verified in source |
| Precedence order is verified by tests: defaults < toml < env vars < explicit overrides | PASS | `env_overrides_toml` test verifies env > toml; `override_beats_env` test verifies overrides > env |
| Missing TOML file produces a warning (via eprintln) and falls back to defaults + env | PASS | `missing_toml_fallback` test verifies both `None` and nonexistent path return Ok(defaults) |
| Double-underscore env var nesting works for at least `ANVILML_FRONTEND__MODE` | PASS | `env_nested_field` test sets ANVILML_FRONTEND__MODE=headless and verifies FrontendMode::Headless |
| `lib.rs` re-exports `load_config`, `ConfigOverrides`, and `ConfigError` from crate root | PASS | `pub use config_load::{ConfigError, ConfigOverrides, load_config};` present in lib.rs |
| No new crate dependencies added to anvilml-core (only std library used) | PASS | Cargo.toml unchanged; only serde, toml, url (existing deps) plus std library |
| No async code, no I/O beyond file read + env vars, zero runtime deps in anvilml-core | PASS | All functions are synchronous; only std::fs::read_to_string and std::env::vars used |
