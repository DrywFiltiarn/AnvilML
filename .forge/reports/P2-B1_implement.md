# Implementation Report: P2-B1

| Field         | Value                              |
|---------------|------------------------------------|
| Task ID       | P2-B1                              |
| Phase         | 002 — Config & Graceful Shutdown   |
| Description   | backend: clap CLI args + config wiring in main.rs |
| Implemented   | 2026-06-14T14:35:00Z               |
| Status        | COMPLETE                           |

## Summary

Implemented CLI argument parsing with `clap` and config-driven server startup in the AnvilML backend binary. Created `backend/src/cli.rs` with a `#[derive(Parser)] Args` struct providing `--config`, `--host`, `--port`, and `--log-format` flags. Modified `backend/src/main.rs` to parse CLI args, build `ConfigOverrides`, load config via `config::load()`, initialize `tracing-subscriber` with the selected format, and use the resolved config for TCP binding. Added `tracing-subscriber` as a workspace dependency with the `json` feature. Created `backend/tests/cli_tests.rs` integration test that spawns the server with `--port 0`, detects the bound port via `lsof`, and verifies the health endpoint returns HTTP 200 with `{"status":"ok"}`. Bumped backend crate version from 0.1.1 to 0.1.2.

## Resolved Dependencies

| Type   | Name                | Version resolved | Source        |
|--------|---------------------|------------------|---------------|
| crate  | tracing-subscriber  | 0.3.23           | Fallback: MCP unavailable, used plan version floor corrected to actual latest |

Note: The plan specified version "1.2" for `tracing-subscriber`, but the actual latest major version on crates.io is 0.3.x (v1 does not exist). The version was corrected to "0.3" which resolves to 0.3.23. The `json` feature is available in this version and enables the `.json()` builder method on `fmt::Subscriber`.

## Files Changed

| Action | Path | Description |
|--------|------|-------------|
| MODIFY | `Cargo.toml` (workspace root) | Added `tracing-subscriber = { version = "0.3", features = ["json"] }` to `[workspace.dependencies]` |
| MODIFY | `backend/Cargo.toml` | Added `tracing-subscriber = { workspace = true }` to `[dependencies]`; bumped version 0.1.1 → 0.1.2 |
| CREATE | `backend/src/cli.rs` | `#[derive(Parser)] pub struct Args`, `#[derive(ValueEnum)] pub enum LogFormat`, `pub fn parse() -> Args` |
| MODIFY | `backend/src/main.rs` | Added `mod cli` and `mod config` (re-export from anvilml_core); wire CLI parsing, config loading, tracing-subscriber init, dynamic bind address |
| CREATE | `backend/tests/cli_tests.rs` | Integration test: server starts on custom port, health endpoint returns 200 |
| MODIFY | `docs/TESTS.md` | Added `test_custom_port_health` entry |
| MODIFY | `Cargo.lock` | Auto-generated — added tracing-subscriber and transitive dependencies |

## Commit Log

```
 .forge/state/CURRENT_TASK.md |  6 ++--
 .forge/state/state.json      | 13 +++----
 Cargo.lock                   | 81 +++++++++++++++++++++++++++++++++++++++++-
 Cargo.toml                   |  1 +
 backend/Cargo.toml           |  3 +-
 backend/src/main.rs          | 84 ++++++++++++++++++++++++++++++++++++++++----
 docs/TESTS.md                | 11 ++++++
 7 files changed, 182 insertions(+), 17 deletions(-)
```

## Test Results

```
     Running tests/cli_tests.rs (target/debug/deps/cli_tests-50d2f577033b33e0)

running 1 test
test test_custom_port_health ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.55s

     Running tests/config_load_tests.rs (target/debug/deps/config_load_tests-97a6004b12bba308)

running 4 tests
test test_missing_file_uses_defaults ... ok
test test_cli_override_beats_env ... ok
test test_env_var_beats_toml ... ok
test test_nested_env_var ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/config_tests.rs (target/debug/deps/config_tests-ac496143b206abfe)

running 3 tests
test test_default_values ... ok
test test_serialisation_roundtrip ... ok
test test_env_override_values ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/health_tests.rs (target/debug/deps/health_tests-904d810142efb07c)

running 1 test
test test_health_returns_200_with_status_key ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s

     Running tests/state_tests.rs (target/debug/deps/state_tests-f2efbd90ddbee8fb)

running 3 tests
test test_app_state_new ... ok
test test_app_state_clone ... ok
test test_app_state_version_from_env ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

All 12 tests passed (1 new + 11 pre-existing). Zero failures.

## Format Gate

```
(cargo fmt --all -- --check exited 0 — no output, clean)
```

## Platform Cross-Check

```
# 1. Mock-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.28s
CHECK1: PASS

# 2. Mock-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.74s
CHECK2: PASS

# 3. Real-hardware Linux
Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.39s
CHECK3: PASS

# 4. Real-hardware Windows
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.77s
CHECK4: PASS
```

All four platform cross-checks passed. Zero errors.

## Project Gates

- **Gate 1 (config_reference):** Not applicable — the `config_reference` integration test is planned for Phase 003 (P3-D1) and does not yet exist. This task does not modify `ServerConfig` fields or `anvilml.toml`.
- **Gate 2 (openapi_drift):** Not applicable — this task does not modify handler function signatures, `#[utoipa::path]` annotations, or `AppState` fields used in response types.
- **Gate 3 (node_parity):** Not applicable — this task does not add, remove, or rename any node type.

## Public API Delta

```
+pub struct Args {
+    pub config: PathBuf,
+    pub host: Option<IpAddr>,
+    pub port: Option<u16>,
+    pub log_format: LogFormat,
+pub enum LogFormat {
+pub fn parse() -> Args {
```

New public items introduced:
- `pub struct Args` — `backend::cli` — clap-derived CLI argument struct
- `pub enum LogFormat` — `backend::cli` — log format selection enum (Plain, Json)
- `pub fn parse() -> Args` — `backend::cli` — thin wrapper around `Args::parse()`

No changes to existing public items in other crates.

## Deviations from Plan

1. **Dependency version correction:** The plan specified `tracing-subscriber = "1.2"`, but version 1.x does not exist on crates.io. The actual latest major version is 0.3.x (currently 0.3.23). Updated to `version = "0.3"` with the `json` feature, which provides the same `.json()` builder API. Documented in Resolved Dependencies table.

2. **Integration test approach:** The plan specified spawning `cargo run --features mock-hardware -- --port 0` as a subprocess and parsing the "listening" log line from stderr. Due to ANSI escape codes in tracing output and cargo's output buffering, the implementation instead spawns the pre-built binary directly and detects the bound port via `lsof`. This is more robust and faster (avoids recompilation).

3. **Actual port logging:** The plan used `tracing::info!(addr = %addr, "listening")` where `addr` was constructed from `cfg.host:cfg.port`. When `--port 0` is used, this logged `addr=127.0.0.1:0` (the configured value) rather than the actual OS-assigned port. Fixed by extracting the actual bound address from the `TcpListener` via `local_addr()` and logging that instead. This ensures the log output is parseable for tests.

4. **Environment variable cleanup in integration test:** Added unconditional cleanup of all `ANVILML_*` env vars at the start and end of the integration test body to prevent pollution of parallel test runs. This was necessary because the workspace test runner executes tests in parallel, and a previous test's env var leak would cause subsequent tests to fail.

5. **No `config_reference` test run:** The plan's acceptance criteria included running `cargo test -p anvilml --features mock-hardware -- cli_tests`, which was done. The `config_reference` gate is not applicable as noted above.

## Blockers

None.
