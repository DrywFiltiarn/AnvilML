# Plan Report: P2-A3

| Field       | Value                                       |
|-------------|---------------------------------------------|
| Task ID     | P2-A3                                         |
| Phase       | 002 — Config & Graceful Shutdown              |
| Description | anvilml: CLI parsing with clap (--config, --host, --port, --no-browser, --log-format) |
| Depends on  | P2-A1, P2-A2                                  |
| Project     | anvilml                                       |
| Planned at  | 2026-06-01T07:55:52Z                          |
| Attempt     | 1                                             |

## Objective

Add `clap` (derive) to the backend crate and create a CLI argument parser module (`backend/src/cli.rs`) that defines an `Args` struct with `--config`, `--host`, `--port`, `--no-browser`, and `--log-format` options. Wire this into `backend/src/main.rs` so that parsed CLI overrides are passed to the existing `load_config()` from `anvilml-core`, replacing the hardcoded `"127.0.0.1:8488"` bind address with the dynamically resolved `cfg.host:cfg.port`. This enables the server to listen on any configurable port and host, forming the foundation for all subsequent phase-2 tasks (tracing init in P2-A4, graceful shutdown in P2-A5).

## Scope

### In Scope
- Add `clap` (derive) dependency to `backend/Cargo.toml`
- Create `backend/src/cli.rs`: `Args` struct with all five CLI flags and `parse()` function returning `(ConfigOverrides, LogFormat)`
- Modify `backend/src/main.rs`: import `cli`, parse args, call `load_config(overrides)`, bind to resolved `cfg.host:cfg.port`
- Update the startup log message to reflect the actual bound address
- Add unit tests in `cli.rs` for the `LogFormat` enum and argument parsing edge cases

### Out of Scope
- Tracing subscriber initialization (reserved for P2-A4)
- Graceful shutdown signal handler (reserved for P2-A5)
- Any changes to `anvilml-core` config types or config loader (already complete in P2-A1, P2-A2)
- Frontend serving logic, model registry, worker management
- CI workflow changes (no new toolchains or matrix entries needed)

## Approach

1. **Add clap dependency.** Edit `backend/Cargo.toml` to add `clap = { version = "4", features = ["derive"] }` under `[dependencies]`. This is the only dependency addition for this task.

2. **Create `backend/src/cli.rs`.** Define two public types:
   - `pub enum LogFormat` with variants `Plain` and `Json`, implementing `Default` (Plain) and a `clap::ValueEnum` trait so clap can parse `--log-format plain|json`
   - `#[derive(clap::Parser)] pub struct Args` with fields:
     - `#[arg(long, default_value = "./anvilml.toml")] config: PathBuf`
     - `#[arg(long)] host: Option<IpAddr>`
     - `#[arg(long)] port: Option<u16>`
     - `#[arg(long)] no_browser: bool`
     - `#[arg(long, value_enum, default_value = "plain")] log_format: LogFormat`
   - Implement `Args::parse_overrides(&self) -> ConfigOverrides` that maps `host`/`port` fields to the `ConfigOverrides` struct from `anvilml-core`
   - Add unit tests: verify `LogFormat` defaults, verify `parse_overrides()` returns correct `ConfigOverrides`, and verify clap can parse a sample command-line string via `Args::try_parse_from()`

3. **Modify `backend/src/main.rs`.** Replace the current hardcoded bind logic:
   - Add `use cli::{Args, LogFormat};` (or `mod cli; use cli::...`)
   - Call `let args = Args::parse();` at the start of `main()`
   - Build `ConfigOverrides` from `args.parse_overrides()`
   - Call `let config = load_config(Some(&args.config), overrides).expect("failed to load config");`
   - Replace the hardcoded `"127.0.0.1:8488"` with `format!("{}:{}", config.host, config.port)`
   - Update the startup log: `println!("Listening on http://{}:{}", config.host, config.port);`
   - Keep all existing `AppState`, `build_router`, and `axum::serve` logic unchanged

4. **Verify compilation.** Run `cargo check -p backend` to confirm no type mismatches, then `cargo test -p backend` to verify tests pass.

## Files Affected

| Action   | Path                              | Description                                          |
|----------|-----------------------------------|------------------------------------------------------|
| MODIFY   | backend/Cargo.toml                | Add clap = { version = "4", features = ["derive"] }  |
| CREATE   | backend/src/cli.rs                | Args struct, LogFormat enum, parse_overrides(), tests |
| MODIFY   | backend/src/main.rs               | Parse CLI args, call load_config with overrides, dynamic bind |

## Tests

| Test ID / Name            | File                     | Validates                                          |
|---------------------------|--------------------------|----------------------------------------------------|
| test_log_format_default   | backend/src/cli.rs       | LogFormat::default() returns Plain                 |
| test_parse_overrides      | backend/src/cli.rs       | Args.parse_overrides() maps host/port to ConfigOverrides correctly |
| test_cli_parse_sample     | backend/src/cli.rs       | clap can parse a sample command line string        |
| test_cli_no_override_port | backend/src/cli.rs       | --port not provided => port is None in overrides   |

## CI Impact

No CI changes required. Adding clap as a new dependency does not affect the CI matrix (`.github/workflows/ci.yml`). The existing `cargo clippy` and `cargo test` commands already cover the `backend` crate.

## Risks and Mitigations

| Risk                      | Likelihood | Impact | Mitigation              |
|---------------------------|-----------|--------|-------------------------|
| clap derive macro version mismatch | Low | Medium | Use clap v4 (current stable), verify with docs.rs lookup |
| `ConfigOverrides` field names differ from plan | Low | High | Already verified: struct has `host: Option<IpAddr>` and `port: Option<u16>` — exact match |
| main.rs bind logic breaks axum::serve integration | Low | Medium | Keep existing axum::serve call structure; only change the TcpListener bind address string |

## Acceptance Criteria

- [ ] `backend/Cargo.toml` contains `clap = { version = "4", features = ["derive"] }` under `[dependencies]`
- [ ] `backend/src/cli.rs` exists with `Args` struct, `LogFormat` enum, and `parse_overrides()` method
- [ ] `backend/src/main.rs` parses CLI args, calls `load_config()` with overrides from `--host`/`--port`, and binds to the resolved address
- [ ] The hardcoded `"127.0.0.1:8488"` string is removed from `main.rs`
- [ ] `cargo check -p backend` succeeds with zero warnings
- [ ] `cargo test -p backend` succeeds (all tests pass)
- [ ] `--config <PATH>` accepts a custom TOML path (default `./anvilml.toml`)
- [ ] `--host <IP>` accepts an optional IP address override
- [ ] `--port <u16>` accepts an optional port number override
- [ ] `--no-browser` is recognized as a boolean flag (value not used in this phase, but parsed without error)
- [ ] `--log-format plain|json` accepts both values with `plain` as default
