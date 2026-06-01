# Implementation Report: P2-A3

| Field          | Value                                      |
|----------------|---------------------------------------------|
| Task ID        | P2-A3                                        |
| Phase          | 002 — Config & Graceful Shutdown              |
| Description    | anvilml: CLI parsing with clap (--config, --host, --port, --no-browser, --log-format) |
| Project        | anvilml                                      |
| Implemented at | 2026-06-01T08:15:00Z                         |
| Attempt        | 1                                            |

## Summary

Added `clap` (derive) to the backend crate and created a CLI argument parser module (`backend/src/cli.rs`) defining an `Args` struct with `--config`, `--host`, `--port`, `--no-browser`, and `--log-format` options. The `LogFormat` enum was defined locally in cli.rs with `Default` (Plain) and `clap::ValueEnum` implementations. Wired this into `backend/src/main.rs` so that parsed CLI overrides are passed to `load_config()` from anvilml-core, replacing the hardcoded `"127.0.0.1:8488"` bind address with the dynamically resolved `cfg.host:cfg.port`. Updated the startup log message to reflect the actual bound address. Added 8 unit tests covering LogFormat variants and Args override mapping.

## Files Changed

| Action   | Path                              | Description                                          |
|----------|-----------------------------------|------------------------------------------------------|
| MODIFY   | backend/Cargo.toml                | Added anvilml-core dependency, added clap v4 derive  |
| CREATE   | backend/src/cli.rs                | Args struct, LogFormat enum, parse() fn, 8 unit tests|
| MODIFY   | backend/src/main.rs               | Import cli module, parse args, resolve config, bind dynamically |
| MODIFY   | Cargo.lock                        | Updated by cargo with new clap/anvilml-core deps     |

## Test Results

### Linux workspace test suite (full)

```
running 9 tests
test config::tests::test_default_server_config ... ok
test config::tests::test_device_type_default ... ok
test config::tests::test_model_kind_default ... ok
test config::tests::test_empty_toml_uses_defaults ... ok
test config_load::tests::env_nested_field ... ok
test config_load::tests::missing_toml_fallback ... ok
test config_load::tests::env_overrides_toml ... ok
test config_load::tests::override_beats_env ... ok
test config::tests::test_toml_roundtrip ... ok

test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 1 test
test tests::health_returns_200 ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

running 8 tests
test cli::tests::test_args_to_overrides_all_none ... ok
test cli::tests::test_args_to_overrides_ipv6 ... ok
test cli::tests::test_args_to_overrides_port_edge ... ok
test cli::tests::test_args_to_overrides_with_values ... ok
test cli::tests::test_log_format_default_is_plain ... ok
test cli::tests::test_log_format_possible_values ... ok
test cli::tests::test_log_format_to_string ... ok
test cli::tests::test_log_format_value_enum_variants ... ok

test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### Windows cross-check (x86_64-pc-windows-gnu)

```
    Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.28s
```

### Clippy (zero warnings)

```
    Checking backend v0.1.0 (/home/dryw/AnvilML/backend)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.30s
```

## CI Changes

No CI changes made.

## Commit Log

```
A  .forge/reports/P2-A3_plan.md
M  .forge/state/CURRENT_TASK.md
M  .forge/state/state.json
M  Cargo.lock
M  backend/Cargo.toml
A  backend/src/cli.rs
M  backend/src/main.rs
```

## Acceptance Criteria — Verification

| Criterion                                          | Status | Evidence                                      |
|----------------------------------------------------|--------|-----------------------------------------------|
| clap v4 with derive feature added to backend/Cargo.toml | PASS | File contains `clap = { version = "4", features = ["derive"] }` |
| backend/src/cli.rs defines Args struct with all 5 flags | PASS | config, host, port, no_browser, log_format fields present |
| backend/src/cli.rs defines LogFormat enum (Plain/Json) with Default and ValueEnum | PASS | derive(Default) + #[default] on Plain + impl clap::ValueEnum |
| main.rs imports cli module and calls parse()        | PASS | `mod cli;` and `let args = cli::parse();` in main() |
| Hardcoded bind address replaced with cfg.host:cfg.port | PASS | `format!("{}:{}", cfg.host, cfg.port)` used for TcpListener::bind |
| Startup log reflects actual bound address           | PASS | `println!("Listening on http://{bind_addr}")` |
| 8 unit tests added and passing                      | PASS | 8 tests in cli::tests module, all pass          |
| cargo fmt --all passes                              | PASS | No formatting changes needed                    |
| cargo clippy --workspace --features mock-hardware -D warnings passes | PASS | Zero warnings                                   |
| cargo check --target x86_64-pc-windows-gnu passes   | PASS | Clean cross-check output                        |
| Full workspace test suite exits 0                   | PASS | 18 tests passed, 0 failed                       |
