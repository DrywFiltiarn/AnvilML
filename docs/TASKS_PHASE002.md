# Tasks: Phase 002 — Config & Graceful Shutdown

| Field | Value |
|-------|-------|
| Phase | 002 |
| Name | Config & Graceful Shutdown |
| Project | anvilml |
| Status | Approved |
| Depends on phases | 1 |

## Overview

Phase 002 adds the two operational foundations that every subsequent phase depends on: a
layered configuration system and a graceful shutdown path. Without config loading, there is
no way to tune ports, paths, or behaviour from the outside. Without graceful shutdown, there
is no safe way to restart the server once workers and database connections exist.

Configuration follows a four-level precedence chain (defaults → toml → env → CLI) as
specified in `ANVILML_DESIGN.md §14`. The `ServerConfig` struct lives in `anvilml-core`
and is populated in `backend/src/main.rs`. The CLI parser uses `clap`. Environment
variables use `ANVILML_` prefix with double-underscore nesting for nested fields.

Graceful shutdown listens for `SIGINT`/`SIGTERM` on Linux and Ctrl-C on Windows via a
cross-platform signal handler, then performs an orderly stop of the axum server. At this
phase there are no workers or database connections to drain, so the shutdown sequence is
simple: stop accepting connections, exit 0.

## Group Reference

| Group | Subsystem | Tasks | Summary |
|-------|-----------|-------|---------|
| A | anvilml-core | P2-A1 … P2-A2 | `ServerConfig` struct + TOML/env loading |
| B | backend | P2-B1 … P2-B2 | CLI (`clap`), graceful shutdown, wiring |

## Prerequisites

Phase 001 complete: binary builds and serves `/health`. `anvilml-core/src/lib.rs` exists.

## Interfaces and Contracts

| Contract document | Relevant tasks | What must match |
|-------------------|---------------|-----------------|
| `ANVILML_DESIGN.md §14` | P2-A1, P2-A2 | `ServerConfig` field names and defaults |
| `ENVIRONMENT.md §4` | P2-A1 | All top-level config fields and their env var names |

## Task Descriptions

### Group A — anvilml-core

#### P2-A1: anvilml-core: ServerConfig struct

**Goal:** Define `ServerConfig` in `crates/anvilml-core/src/config.rs` with all fields per `ANVILML_DESIGN.md §14` and `ENVIRONMENT.md §4`. Implement `Default`.

**Files to create:**
- `crates/anvilml-core/src/config.rs` — `ServerConfig` with fields: `host`, `port`, `db_path`, `artifact_dir`, `num_threads`, `venv_path`, `max_ipc_payload_mib`, `seeds_path`, `model_dirs`, `gpu_selection`, `limits`, `rocm`, `hardware_override`. All nested structs in same file. `Default` impl uses documented defaults. `Serialize`/`Deserialize` via serde.

**Acceptance criterion:** `cargo test -p anvilml-core -- config` exits 0 with ≥ 3 tests (default values, serialisation roundtrip, env override).

#### P2-A2: anvilml-core: config loading (toml + env override)

**Goal:** Implement `pub fn load(path: &Path, overrides: &ConfigOverrides) -> Result<ServerConfig>` in `crates/anvilml-core/src/config_load.rs` applying the four-level precedence chain.

**Files to create:**
- `crates/anvilml-core/src/config_load.rs` — read toml file (or use defaults if absent), apply `ANVILML_*` env vars, apply `ConfigOverrides`. Return `ServerConfig`. Add `toml` crate as workspace dep.

**Acceptance criterion:** `cargo test -p anvilml-core -- config_load` exits 0 with ≥ 4 tests covering: missing file uses defaults, env var overrides toml, CLI override beats env.

### Group B — backend

#### P2-B1: backend: clap CLI + config wiring

**Goal:** Implement `backend/src/cli.rs` with `clap`-derived `Args` struct (`--config`, `--host`, `--port`, `--log-format`). Wire into `main.rs` to load config before binding.

**Files to create:**
- `backend/src/cli.rs` — `#[derive(Parser)] struct Args` with the four flags. `pub fn parse() -> Args`.
**Files to modify:**
- `backend/src/main.rs` — call `cli::parse()`, call `config::load()`, use `cfg.port` for bind address.

**Acceptance criterion:** `cargo run --features mock-hardware -- --port 9001 &` + `curl :9001/health` → 200; kill.

#### P2-B2: backend: cross-platform graceful shutdown

**Goal:** Implement `backend/src/shutdown.rs` providing `pub async fn shutdown_signal()` that resolves on SIGINT/SIGTERM (Unix) or Ctrl-C (Windows). Wire into `axum::serve().with_graceful_shutdown()`.

**Files to create:**
- `backend/src/shutdown.rs` — `#[cfg(unix)]` uses `tokio::signal::unix`; `#[cfg(windows)]` uses `tokio::signal::ctrl_c`. Logs INFO "shutdown signal received" with the signal name.
**Files to modify:**
- `backend/src/main.rs` — pass `shutdown_signal()` to `axum::serve(...).with_graceful_shutdown(...)`.

**Acceptance criterion:** Start server; send SIGTERM (Linux) or Ctrl-C (Windows); process exits 0 within 3 seconds; log line "shutdown signal received" visible.

## Phase Acceptance Criteria

```bash
cargo build --features mock-hardware
cargo run --features mock-hardware -- --port 9001 &
sleep 2
curl -s http://127.0.0.1:9001/health | python3 -c "import sys,json; d=json.load(sys.stdin); assert d['status']=='ok'"
kill -SIGTERM %1
wait %1
echo "Exit code: $?"
```

## Known Constraints and Gotchas

- The `ConfigOverrides` struct is separate from `ServerConfig`. It carries only the values the CLI can override (host, port) and is merged last in the precedence chain.
- The `config_reference` integration test (`backend/tests/config_reference.rs`) will be introduced in Phase 003 together with the full type suite. Phase 002 lays the `ServerConfig` groundwork that test will validate.
- `tokio::signal` requires the `signal` feature in `tokio`. Add it to the workspace dep.
