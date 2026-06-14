# Plan Report: P2-B1

| Field       | Value                                             |
|-------------|---------------------------------------------------|
| Task ID     | P2-B1                                             |
| Phase       | 002 — Config & Graceful Shutdown                  |
| Description | backend: clap CLI args + config wiring in main.rs |
| Depends on  | P2-A1, P2-A2                                      |
| Project     | anvilml                                           |
| Planned at  | 2026-06-14T13:30:00Z                              |
| Attempt     | 1                                                 |

## Objective

Create `backend/src/cli.rs` with a `clap`-derived `Args` struct providing `--config`, `--host`, `--port`, and `--log-format` flags, then modify `backend/src/main.rs` to parse CLI arguments, build `ConfigOverrides`, call `config::load()`, and use the loaded `ServerConfig` for the TCP listener bind address. After this task, a developer can run `cargo run --features mock-hardware -- --port 9001` and verify the health endpoint responds with HTTP 200 at `http://127.0.0.1:9001/health`.

## Scope

### In Scope
- Create `backend/src/cli.rs`: `#[derive(Parser)] Args` struct with `--config(PathBuf)`, `--host(Option<IpAddr>)`, `--port(Option<u16>)`, `--log-format(LogFormat)` fields. `pub fn parse() -> Args`.
- Modify `backend/src/main.rs`: call `cli::parse()`, build `ConfigOverrides` from args, call `config::load()`, log loaded config, initialise `tracing-subscriber` with the selected format, use `cfg.host`/`cfg.port` for `TcpListener` bind.
- Add `tracing-subscriber` as a workspace dependency with `json` feature.
- Create `backend/tests/cli_tests.rs`: integration test verifying the server starts on a custom port and responds to `GET /health` with HTTP 200.
- Bump `backend` crate patch version in `backend/Cargo.toml` (0.1.1 → 0.1.2).

### Out of Scope
- Graceful shutdown signal handling (P2-B2).
- Database migration or seed loading (later phases).
- Worker spawn/supervision (later phases).
- Any changes to `anvilml-core` — `ServerConfig`, `ConfigOverrides`, and `config::load()` are already implemented by P2-A1/P2-A2.
- `#[tracing::instrument]` on `main()` — not appropriate for the entry point function.

## Existing Codebase Assessment

Phase 002 has already completed P2-A1 and P2-A2. `crates/anvilml-core/src/config.rs` defines `ServerConfig` with all fields, nested structs (`ModelDirConfig`, `GpuSelectionConfig`, `LimitsConfig`, `RocmConfig`, `HardwareOverrideConfig`), `Default` impl, and `Serialize`/`Deserialize` derives. `crates/anvilml-core/src/config_load.rs` provides `pub fn load(path: &Path, overrides: &ConfigOverrides) -> Result<ServerConfig, AnvilError>` implementing the four-level precedence chain. `ConfigOverrides` carries only `host: Option<String>` and `port: Option<u16>`. `crates/anvilml-core/src/lib.rs` re-exports `load`, `ConfigOverrides`, and `ServerConfig`.

`backend/src/main.rs` currently hardcodes `127.0.0.1:8488` and creates `AppState` with `env!("CARGO_PKG_VERSION")`, builds the router, binds, logs at INFO, and runs the server — no CLI parsing or config loading. `backend/Cargo.toml` already declares `clap = { version = "4.5.54", features = ["derive"] }` as a direct dependency. There is no `cli.rs`, no `shutdown.rs`, and no `backend/tests/` directory yet.

The established patterns include: `///` doc comments on all public items, structured tracing log calls with field notation (`tracing::info!(addr = %addr, ...)`), `AnvilError` for error handling, `#[serde(default)]` for optional config fields, and `Result<T, AnvilError>` for fallible operations. Tests go in `crates/{name}/tests/` or `backend/tests/`.

## Resolved Dependencies

| Type   | Name                | Version verified | MCP source     | Feature flags confirmed |
|--------|---------------------|------------------|----------------|------------------------|
| crate  | clap                | 4.6.1 (via 4.5.54 in Cargo.toml) | Cargo.lock + backend/Cargo.toml | derive (already present) |
| crate  | tracing-subscriber  | 1.2 (stable, mid-2025) | Fallback: Cargo.lock absent — MCP `rust-docs` unavailable. Use version floor ≥ 1.2. | json |

Note: `clap` is already declared in `backend/Cargo.toml` (not a new dependency). The Cargo.lock resolves it to 4.6.1. The `Parser` derive macro, `ArgAction`, and `IpAddr`/`u16`/`PathBuf` value types are all stable in clap 4.x. `IpAddr` implements `ValueParserFactory` in clap 4.x, so `Option<IpAddr>` parses and validates automatically. `tracing-subscriber` is not yet in the workspace — it must be added.

## Approach

1. **Add `tracing-subscriber` to workspace dependencies.** Append `tracing-subscriber = { version = "1.2", features = ["json"] }` to `[workspace.dependencies]` in the root `Cargo.toml`. The `json` feature enables the `JsonSubscriber` builder for JSON log output. This is the only new external dependency this task introduces.

2. **Add `tracing-subscriber` to backend dependencies.** In `backend/Cargo.toml`, add `tracing-subscriber = { workspace = true }` under `[dependencies]`. This keeps version management centralized in the workspace.

3. **Create `backend/src/cli.rs`.** Define:
   - `#[derive(clap::Parser, Debug)] pub struct Args` with four fields:
     - `#[arg(long, default_value = "./anvilml.toml")] pub config: std::path::PathBuf` — path to TOML config file. The default matches the documented default in ENVIRONMENT.md §4.
     - `#[arg(long)] pub host: Option<std::net::IpAddr>` — optional bind address override. Clap validates `IpAddr` via its `ValueParserFactory` impl.
     - `#[arg(long)] pub port: Option<u16>` — optional port override.
     - `#[arg(long, value_enum, default_value = "plain")] pub log_format: LogFormat` — log output format.
   - `#[derive(clap::ValueEnum, Clone, Debug, PartialEq)] pub enum LogFormat { Plain, Json }` — simple enum for the two format options.
   - `pub fn parse() -> Args` — simply calls `Args::parse()`. This is a thin wrapper that makes the function name match the task spec and allows testing without consuming `std::env::args()`.

   The struct gets a `#[clap(name = "anvilml", about = "AnvilML server")]` attribute for clean help output.

4. **Modify `backend/src/main.rs`.** Replace the current hardcoded bind address with config-driven logic:
   - Call `let args = cli::parse();` at the start of `main()`.
   - Build `ConfigOverrides`:
     ```rust
     let overrides = anvilml_core::ConfigOverrides {
         host: args.host.map(|ip| ip.to_string()),
         port: args.port,
     };
     ```
     This converts `Option<IpAddr>` to `Option<String>` to match `ConfigOverrides`'s field type. The `IpAddr::to_string()` call produces a valid string representation that `ConfigOverrides` stores directly.
   - Call `let cfg = config::load(&args.config, &overrides).expect("failed to load config");` to load the full configuration.
   - Add a mandatory INFO log point per ENVIRONMENT.md §9: `tracing::info!(host = %cfg.host, port = %cfg.port, "config loaded");` — this logs the operational state after config resolution.
   - Initialise the tracing subscriber based on `args.log_format`:
     ```rust
     match args.log_format {
         cli::LogFormat::Plain => {
             tracing_subscriber::fmt::Subscriber::builder()
                 .with_max_level(tracing::Level::INFO)
                 .init();
         }
         cli::LogFormat::Json => {
             tracing_subscriber::fmt::Subscriber::builder()
                 .with_max_level(tracing::Level::INFO)
                 .json()
                 .init();
         }
     }
     ```
     The `tracing::Level::INFO` filter matches the default log level from ENVIRONMENT.md §3.1 (ANVILML_LOG defaults to `info`). This is a non-obvious choice: we use `fmt::Subscriber` for both formats rather than the separate `tracing-subscriber-json` crate, because the `json()` builder method on `fmt::Subscriber` is the standard approach and avoids an additional dependency.
   - Build `cfg.host:cfg.port` for the bind address:
     ```rust
     let addr = format!("{}:{}", cfg.host, cfg.port);
     ```
     This replaces the hardcoded `"127.0.0.1:8488"`. The `cfg.port` is a `u16` and `cfg.host` is a `String`, so `format!` produces the correct address string.
   - Keep the existing `TcpListener::bind(addr)`, `tracing::info!(addr = %addr, "listening")`, and `axum::serve(...)` lines unchanged.

5. **Create `backend/tests/cli_tests.rs`.** Write an integration test that:
   - Spawns `cargo run --features mock-hardware -- --port 0` as a subprocess (port 0 for OS-assigned port, per test isolation rules).
   - Reads the actual port from the server's log output (`"listening"` log line contains the port).
   - Sends `GET /health` to the OS-assigned port.
   - Asserts the response is HTTP 200 with `"status":"ok"`.
   
   This test verifies the full config-loading path: CLI parsing → ConfigOverrides → config::load → TCP bind → health endpoint.

6. **Bump backend crate version.** Change `version = "0.1.1"` to `version = "0.1.2"` in `backend/Cargo.toml` per §14 of FORGE_AGENT_RULES and §12 of ENVIRONMENT.md.

## Public API Surface

| Item | Type | Module Path | Signature |
|------|------|-------------|-----------|
| `Args` | `pub struct` (derived `Parser`) | `backend::cli` | `#[derive(clap::Parser)] pub struct Args { config: PathBuf, host: Option<IpAddr>, port: Option<u16>, log_format: LogFormat }` |
| `LogFormat` | `pub enum` (derived `ValueEnum`) | `backend::cli` | `#[derive(clap::ValueEnum)] pub enum LogFormat { Plain, Json }` |
| `parse` | `pub fn` | `backend::cli` | `pub fn parse() -> Args` |

No changes to any existing `pub` items in other crates. `ConfigOverrides`, `ServerConfig`, and `config::load` are already public from P2-A1/P2-A2.

## Files Affected

| Action | Path | Description |
|--------|------|-------------|
| CREATE | `backend/src/cli.rs` | clap-derived Args struct, LogFormat enum, parse() function |
| MODIFY | `backend/src/main.rs` | Wire CLI parsing, config loading, tracing-subscriber init, dynamic bind address |
| MODIFY | `Cargo.toml` (workspace root) | Add `tracing-subscriber` workspace dependency |
| MODIFY | `backend/Cargo.toml` | Add `tracing-subscriber = { workspace = true }` dep; bump version 0.1.1 → 0.1.2 |
| CREATE | `backend/tests/cli_tests.rs` | Integration test: server starts on custom port, health endpoint returns 200 |

## Tests

| Test File | Test Name | What It Verifies | Preconditions | Inputs | Expected Output | Acceptance Command |
|-----------|-----------|-----------------|---------------|--------|----------------|--------------------|
| `backend/tests/cli_tests.rs` | `test_custom_port_health` | Server accepts `--port 9001`, binds to it, and `GET /health` returns HTTP 200 with `"status":"ok"` | Workspace builds with `mock-hardware`; no prior server running on port 9001 | `cargo run --features mock-hardware -- --port 9001` as subprocess | HTTP 200 response with JSON body containing `"status":"ok"` | `cargo test -p anvilml --features mock-hardware -- cli_tests` exits 0 |

The test spawns the binary as a subprocess, waits for the "listening" log line to confirm the server is ready, sends an HTTP request to the bound address, and asserts the response. The subprocess is terminated after the assertion.

## CI Impact

No CI changes required. The new `backend/tests/cli_tests.rs` file is picked up automatically by the existing `cargo test --workspace --features mock-hardware` CI job (both `rust-linux` and `rust-windows` runners). The `config-drift` CI job (`cargo test -p backend --features mock-hardware -- config_reference`) is unaffected since this task does not modify `ServerConfig` fields or `anvilml.toml`.

## Platform Considerations

None identified. The `clap` derive macros, `tracing-subscriber::fmt`, and `std::net::IpAddr` are all cross-platform. The `TcpListener` bind address format (`host:port`) is identical on Linux and Windows. No `#[cfg(unix)]` or `#[cfg(windows)]` guards are needed. The Windows cross-check in ENVIRONMENT.md §7 is sufficient.

## Risks and Mitigations

| Risk | Likelihood | Impact | Mitigation |
|------|------------|--------|------------|
| `tracing-subscriber` version 1.2 API may differ from what the plan assumes — specifically, the `.json()` builder method signature or the `.init()` method availability. | Medium | High | Before writing code, verify that `tracing-subscriber::fmt::Subscriber::builder().json().init()` compiles. If the API differs, use the matching builder chain for the resolved version. |
| `IpAddr::to_string()` on an `Ipv6Addr` produces `[::1]:port` which may not be a valid bind address for `TcpListener::bind` in all cases. | Low | Medium | The `host` default from `ServerConfig::default()` is `"127.0.0.1"` (IPv4). If the user provides an IPv6 address, the format string `"{host}:{port}"` produces `[::1]:port` which `TcpListener::bind` accepts. No fix needed — this is standard behaviour. Document the assumption in an inline comment. |
| `config::load()` may fail (e.g. malformed TOML file at the path from `--config`), and the current `main.rs` uses `.expect()` which panics. This is acceptable for a binary entry point but produces a non-user-friendly error. | Low | Low | Use `.expect("failed to load config")` which produces a clear error message with the underlying error chain via `Debug`. This matches the existing pattern in `main.rs` where `.expect("failed to bind listener")` is already used. |
| The `tracing-subscriber` dependency is not in the Cargo.lock (it does not exist yet), so the version is a best-guess from MCP unavailability. The ACT agent must resolve the exact version at session start. | High | High | Documented in Resolved Dependencies table. ACT agent must query `rust-docs` MCP for the latest `tracing-subscriber` version and use it. Record the resolved version in the implementation report. |

## Acceptance Criteria

- [ ] `cargo build --workspace --features mock-hardware` exits 0
- [ ] `cargo test -p anvilml --features mock-hardware -- cli_tests` exits 0
- [ ] `cargo run --features mock-hardware -- --port 9001 &` starts the server; `sleep 2` then `curl -s http://127.0.0.1:9001/health | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['status']=='ok'"` exits 0
- [ ] `kill %1 && wait %1` — the server process exits cleanly after SIGTERM (or Ctrl-C)
- [ ] `cargo fmt --all -- --check` exits 0 (format gate)
- [ ] `cargo clippy --workspace --features mock-hardware -- -D warnings` exits 0 (lint gate)
