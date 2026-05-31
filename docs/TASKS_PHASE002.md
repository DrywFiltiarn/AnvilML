# Tasks: Phase 002 — Config & Graceful Shutdown

| Field | Value |
|-------|-------|
| Phase | 002 |
| Name | Config & Graceful Shutdown |
| Milestone group | Runnable server skeleton |
| Depends on phases | 1 |
| Task file | `forge/tasks/tasks_phase002.json` |
| Tasks | 5 |

## Overview

Phase 2 makes the running binary configurable and well-behaved on exit. It adds the layered `ServerConfig` (defaults -> `anvilml.toml` -> `ANVILML_*` env -> CLI flags), `clap` CLI parsing, structured `tracing` logging, and a cross-platform graceful shutdown handler. After this phase you can start the server on any port and stop it cleanly with Ctrl-C.

Every task in this phase implements **one module or one endpoint** plus its test. No task touches more than its named file(s). `cargo test` and `cargo clippy` are per-task gates; the phase as a whole is only complete when the **Runnable Proof** below passes.

## Tasks

| Task | Module / File | Summary |
|------|-------------|---------|
| P2-A1 | `crates/anvilml-core/src/config.rs` | anvilml-core: ServerConfig types with defaults |
| P2-A2 | `crates/anvilml-core/src/config_load.rs` | anvilml-core: layered config loader (defaults -> toml -> env -> overrides) |
| P2-A3 | `backend/src/cli.rs` | anvilml: CLI parsing with clap (--config, --host, --port, --no-browser, --log-format) |
| P2-A4 | `backend/src/main.rs` | anvilml: tracing subscriber init (plain/json, ANVILML_LOG env filter) |
| P2-A5 | `backend/src/shutdown.rs` | anvilml: cross-platform graceful shutdown signal handler |

## Task details

#### P2-A1: anvilml-core: ServerConfig types with defaults

- **Prereqs:** P1-A5
- **Tags:** —

In anvilml-core add serde+toml deps. Create src/config.rs: ServerConfig and nested ModelDirConfig, RocmConfig, HardwareOverrideConfig, FrontendConfig(+FrontendMode enum Local/Remote/Headless), GpuSelectionConfig, LimitsConfig per ANVILML_DESIGN 3.1. Every field has a documented default via Default impl or serde(default). db_path default './anvilml.db'. Derive Deserialize, Serialize, Clone, Debug. Re-export from lib.rs. cargo test -p anvilml-core -- config exits 0 with a TOML round-trip test.

#### P2-A2: anvilml-core: layered config loader (defaults -> toml -> env -> overrides)

- **Prereqs:** P2-A1
- **Tags:** reasoning

Create src/config_load.rs: fn load_config(toml_path: Option<&Path>, overrides: ConfigOverrides) -> Result<ServerConfig>. Precedence low->high: built-in defaults, anvilml.toml (warn+skip if absent), ANVILML_* env vars (double underscore for nested e.g. ANVILML_FRONTEND__MODE), then explicit overrides (host/port). ConfigOverrides struct holds Option<IpAddr> host, Option<u16> port. cargo test -p anvilml-core -- config_load exits 0: env overrides toml, override beats env.

#### P2-A3: anvilml: CLI parsing with clap (--config, --host, --port, --no-browser, --log-format)

- **Prereqs:** P2-A2
- **Tags:** —

Add clap (derive) to backend. Create backend/src/cli.rs: Args struct with --config <PATH> (default ./anvilml.toml), --host <IP>, --port <u16>, --no-browser (flag), --log-format plain|json (default plain). In main.rs parse Args, call load_config with overrides from --host/--port. Bind to cfg.host:cfg.port (not hardcoded). Verify: cargo run -- --port 9000 binds 9000; curl http://127.0.0.1:9000/health works.

#### P2-A4: anvilml: tracing subscriber init (plain/json, ANVILML_LOG env filter)

- **Prereqs:** P2-A3
- **Tags:** —

Add tracing + tracing-subscriber (features env-filter, json) to backend. In main.rs init subscriber before anything else: EnvFilter from ANVILML_LOG then RUST_LOG, default info. --log-format plain uses fmt(), json uses fmt().json(). Replace the println startup logs with tracing::info!. Verify: ANVILML_LOG=debug cargo run shows debug lines; cargo run -- --log-format json emits JSON log lines.

#### P2-A5: anvilml: cross-platform graceful shutdown signal handler

- **Prereqs:** P2-A4
- **Tags:** reasoning

Create backend/src/shutdown.rs: async fn shutdown_signal() joining tokio::signal::ctrl_c() with #[cfg(unix)] SIGTERM (tokio::signal::unix) and #[cfg(windows)] ctrl_close+ctrl_shutdown (tokio::signal::windows), using std::future::pending for the inactive-platform arms so it compiles everywhere. Pass it to axum::serve(...).with_graceful_shutdown(shutdown_signal()). On signal log 'Shutting down' and exit 0 after serve returns. Verify: cargo run then Ctrl-C prints shutting down and exits 0 within 1s. Also pass: cargo check --target x86_64-pc-windows-gnu --features mock-hardware.


## Runnable Proof

Start on a custom port, confirm it binds there, then shut down cleanly.

```bash
cargo run -- --port 9000 --log-format json
# another terminal:
curl -s http://127.0.0.1:9000/health        # 200
# back in the server terminal, press Ctrl-C
```

Expected: server logs `Listening on http://127.0.0.1:9000`; the curl succeeds on 9000; pressing Ctrl-C logs `Shutting down` and the process exits 0 within ~1s (no hang, no panic). Also verify `ANVILML_PORT=9100 cargo run` binds 9100 (env override) and `--port 9200` beats the env var.
